function stepThree(): void {
  console.log("three");
}

function stepTwo(): void {
  stepThree();
}

function stepOne(): void {
  stepTwo();
}

export function runPipeline(): void {
  stepOne();
  exportedStep();
}

export function exportedStep(): void {
  console.log("exported");
  leafStep();
}

function leafStep(): void {
  console.log("leaf");
}
