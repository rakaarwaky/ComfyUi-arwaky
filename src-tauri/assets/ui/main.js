// Status text rotation to give the user details on what is happening
const statusSteps = [
    { time: 0, text: "Booting launcher environment...", progress: 10 },
    { time: 2000, text: "Starting ComfyUI Python backend...", progress: 25 },
    { time: 4000, text: "Configuring GPU device (Radeon RX 6800 XT)...", progress: 45 },
    { time: 6000, text: "Loading PyTorch and ROCm kernels...", progress: 65 },
    { time: 9000, text: "Waiting for server to listen on port 8188...", progress: 85 },
    { time: 15000, text: "ROCm initialization taking longer than usual...", progress: 90 },
    { time: 25000, text: "Checking for active backend instance...", progress: 95 }
];

const statusTextEl = document.getElementById("status-text");
const progressFillEl = document.getElementById("progress-fill");

// Initialize status rotation
statusSteps.forEach(step => {
    setTimeout(() => {
        if (statusTextEl && progressFillEl) {
            statusTextEl.textContent = step.text;
            progressFillEl.style.width = `${step.progress}%`;
        }
    }, step.time);
});
