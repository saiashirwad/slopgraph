use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::Error;

pub const KIND_CALL_EXPRESSION: u32 = 214;
const HEADER_OFFSET_NODES: usize = 40;
const NODE_LEN: usize = 28;

/// One node in a tsgo source-file binary AST.
#[derive(Debug, Clone)]
pub struct TsNode {
    pub index: u32,
    pub kind: u32,
    pub pos: u32,
    pub end: u32,
}

pub struct Tsgo {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
    pub case_sensitive: bool,
}

impl Tsgo {
    pub fn spawn(cwd: &Path) -> Result<Self, Error> {
        let exe = find_tsgo()?;
        let mut child = Command::new(&exe)
            .args(["--api", "--async", "--cwd"])
            .arg(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::Tsgo(format!("spawn {}: {e}", exe.display())))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::Tsgo("tsgo stdin missing".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::Tsgo("tsgo stdout missing".into()))?;
        let mut session = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 0,
            case_sensitive: true,
        };
        let init = session.request("initialize", Value::Null)?;
        session.case_sensitive = init
            .get("useCaseSensitiveFileNames")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        Ok(session)
    }

    pub fn canonical_path(&self, path: &Path) -> String {
        let s = path.to_string_lossy().replace('\\', "/");
        if self.case_sensitive {
            s
        } else {
            s.to_lowercase()
        }
    }

    pub fn open_project(&mut self, tsconfig: &Path) -> Result<Snapshot, Error> {
        let params = json!({ "openProjects": [tsconfig.to_string_lossy()] });
        let data = self.request("updateSnapshot", params)?;
        let parsed: SnapshotWire = serde_json::from_value(data)
            .map_err(|e| Error::Tsgo(format!("updateSnapshot: {e}")))?;
        let project = parsed
            .projects
            .first()
            .ok_or_else(|| Error::Tsgo("updateSnapshot returned no project".into()))?;
        Ok(Snapshot {
            id: parsed.snapshot,
            project: project.id.clone(),
        })
    }

    pub fn source_nodes(&mut self, snap: &Snapshot, file: &Path) -> Result<Vec<TsNode>, Error> {
        let params = json!({
            "snapshot": snap.id,
            "project": snap.project,
            "file": file.to_string_lossy(),
        });
        let data = self.request("getSourceFile", params)?;
        let encoded = data
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Tsgo("getSourceFile missing data".into()))?;
        let bytes = decode_base64(encoded)?;
        parse_nodes(&bytes)
    }

    pub fn resolved_signature(
        &mut self,
        snap: &Snapshot,
        location: &str,
    ) -> Result<Option<Signature>, Error> {
        let params = json!({
            "snapshot": snap.id,
            "project": snap.project,
            "location": location,
        });
        match self.request("getResolvedSignature", params) {
            Ok(Value::Null) | Ok(Value::Bool(false)) => Ok(None),
            Ok(value) => {
                let sig: Signature = serde_json::from_value(value)
                    .map_err(|e| Error::Tsgo(format!("getResolvedSignature: {e}")))?;
                Ok(Some(sig))
            }
            Err(Error::Tsgo(msg)) if msg.contains("could not be resolved") => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, Error> {
        self.next_id += 1;
        let id = self.next_id;
        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let body = payload.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{body}", body.len())
            .and_then(|_| self.stdin.flush())
            .map_err(|e| Error::Tsgo(format!("write {method}: {e}")))?;

        let msg = read_rpc(&mut self.stdout)?;
        if msg.get("id").and_then(Value::as_i64) != Some(id) {
            return Err(Error::Tsgo(format!(
                "{method}: unexpected rpc id {:?}",
                msg.get("id")
            )));
        }
        if let Some(err) = msg.get("error") {
            let message = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("rpc error");
            return Err(Error::Tsgo(message.to_string()));
        }
        Ok(msg.get("result").cloned().unwrap_or(Value::Null))
    }
}

impl Drop for Tsgo {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Snapshot {
    pub id: i64,
    pub project: String,
}

#[derive(Debug, Deserialize)]
pub struct Signature {
    pub declaration: Option<String>,
}

impl Signature {
    pub fn is_resolved(&self) -> bool {
        self.declaration.as_deref().is_some_and(|d| !d.is_empty())
    }
}

#[derive(Deserialize)]
struct SnapshotWire {
    snapshot: i64,
    projects: Vec<ProjectWire>,
}

#[derive(Deserialize)]
struct ProjectWire {
    id: String,
}

pub fn node_handle(index: u32, kind: u32, canonical_path: &str) -> String {
    format!("{index}.{kind}.{canonical_path}")
}

pub fn parse_handle(handle: &str) -> Option<(u32, u32, String)> {
    let first = handle.find('.')?;
    let rest = &handle[first + 1..];
    let second = rest.find('.')?;
    let index = handle[..first].parse().ok()?;
    let kind = rest[..second].parse().ok()?;
    Some((index, kind, rest[second + 1..].to_string()))
}

pub fn tightest_containing<'a>(
    nodes: &'a [TsNode],
    kinds: &[u32],
    utf16: u32,
) -> Option<&'a TsNode> {
    nodes
        .iter()
        .filter(|n| kinds.contains(&n.kind) && n.pos <= utf16 && utf16 < n.end)
        .min_by_key(|n| n.end.saturating_sub(n.pos))
}

pub fn node_by_index(nodes: &[TsNode], index: u32) -> Option<&TsNode> {
    nodes.iter().find(|n| n.index == index)
}

pub fn utf16_offset(source: &str, utf8: u32) -> u32 {
    let mut off = (utf8 as usize).min(source.len());
    while off > 0 && !source.is_char_boundary(off) {
        off -= 1;
    }
    source[..off].encode_utf16().count() as u32
}

fn parse_nodes(bytes: &[u8]) -> Result<Vec<TsNode>, Error> {
    if bytes.len() < HEADER_OFFSET_NODES + 4 {
        return Err(Error::Tsgo("source file binary too small".into()));
    }
    let offset = u32::from_le_bytes(
        bytes[HEADER_OFFSET_NODES..HEADER_OFFSET_NODES + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    if offset > bytes.len() {
        return Err(Error::Tsgo("invalid node table offset".into()));
    }
    let count = (bytes.len() - offset) / NODE_LEN;
    let mut nodes = Vec::with_capacity(count.saturating_sub(1));
    for i in 1..count {
        let base = offset + i * NODE_LEN;
        if base + 12 > bytes.len() {
            break;
        }
        let kind = u32::from_le_bytes(bytes[base..base + 4].try_into().unwrap());
        let pos = i32::from_le_bytes(bytes[base + 4..base + 8].try_into().unwrap());
        let end = i32::from_le_bytes(bytes[base + 8..base + 12].try_into().unwrap());
        if kind == 0xFFFF_FFFF {
            continue;
        }
        nodes.push(TsNode {
            index: i as u32,
            kind,
            pos: pos.max(0) as u32,
            end: end.max(0) as u32,
        });
    }
    Ok(nodes)
}

fn decode_base64(input: &str) -> Result<Vec<u8>, Error> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut buf = 0u32;
    let mut n = 0;
    for &c in bytes {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let Some(v) = val(c) else {
            return Err(Error::Tsgo("invalid base64 in getSourceFile".into()));
        };
        buf = (buf << 6) | u32::from(v);
        n += 1;
        if n == 4 {
            out.push((buf >> 16) as u8);
            out.push((buf >> 8) as u8);
            out.push(buf as u8);
            buf = 0;
            n = 0;
        }
    }
    match n {
        0 => {}
        2 => out.push((buf >> 4) as u8),
        3 => {
            out.push((buf >> 10) as u8);
            out.push((buf >> 2) as u8);
        }
        _ => return Err(Error::Tsgo("invalid base64 in getSourceFile".into())),
    }
    Ok(out)
}

fn read_rpc(reader: &mut BufReader<ChildStdout>) -> Result<Value, Error> {
    let mut length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| Error::Tsgo(format!("read header: {e}")))?;
        if n == 0 {
            return Err(Error::Tsgo("tsgo closed stdout".into()));
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.to_ascii_lowercase().strip_prefix("content-length:") {
            length = rest.trim().parse().ok();
        }
    }
    let length = length.ok_or_else(|| Error::Tsgo("missing Content-Length".into()))?;
    let mut buf = vec![0; length];
    reader
        .read_exact(&mut buf)
        .map_err(|e| Error::Tsgo(format!("read body: {e}")))?;
    serde_json::from_slice(&buf).map_err(|e| Error::Tsgo(format!("rpc json: {e}")))
}

fn find_tsgo() -> Result<PathBuf, Error> {
    if let Some(path) = std::env::var_os("SLOPGRAPH_TSGO") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(resolve_native(&path).unwrap_or(path));
        }
    }
    if let Some(found) = search_path("tsgo").or_else(|| search_path("tsc")) {
        return Ok(resolve_native(&found).unwrap_or(found));
    }
    Err(Error::TsgoNotFound)
}

fn search_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let mut candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if cfg!(windows) {
            candidate.set_extension("exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn resolve_native(path: &Path) -> Option<PathBuf> {
    if looks_native(path) {
        return Some(path.to_path_buf());
    }
    let plat = platform_package()?;
    let bin_dir = path.parent()?;
    // typescript/bin/tsc -> typescript/node_modules/@typescript/<plat>/lib/tsc
    let pkg = bin_dir.parent()?;
    [
        pkg.join("node_modules")
            .join("@typescript")
            .join(plat)
            .join("lib")
            .join("tsc"),
        pkg.join("node_modules")
            .join("@typescript")
            .join(plat)
            .join("lib")
            .join("tsgo"),
        pkg.join("node_modules").join(plat).join("lib").join("tsc"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

fn looks_native(path: &Path) -> bool {
    let mut buf = [0u8; 1];
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    matches!(file.read(&mut buf), Ok(1) if buf[0] != b'#')
}

fn platform_package() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "typescript-darwin-arm64",
        ("macos", "x86_64") => "typescript-darwin-x64",
        ("linux", "aarch64") => "typescript-linux-arm64",
        ("linux", "x86_64") => "typescript-linux-x64",
        ("windows", "aarch64") => "typescript-win32-arm64",
        ("windows", "x86_64") => "typescript-win32-x64",
        _ => return None,
    })
}
