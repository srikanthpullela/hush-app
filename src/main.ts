import { invoke } from "@tauri-apps/api/core";

const autoSetup = document.getElementById("auto-setup")!;
const manualSetup = document.getElementById("manual-setup")!;
const allDone = document.getElementById("all-done")!;
const checkOn = document.getElementById("check-on")!;
const checkOff = document.getElementById("check-off")!;
const stepOn = document.getElementById("step-on")!;
const stepOff = document.getElementById("step-off")!;
const doneBtn = document.getElementById("done-btn") as HTMLButtonElement;
const openBtn = document.getElementById("open-shortcuts-btn")!;

let pollTimer: number | null = null;

async function checkShortcuts(): Promise<{ on: boolean; off: boolean }> {
  const result = await invoke<{ has_on: boolean; has_off: boolean }>(
    "check_shortcuts"
  );
  return { on: result.has_on, off: result.has_off };
}

function updateChecks(on: boolean, off: boolean) {
  // Hush On
  if (on) {
    checkOn.textContent = "✓";
    checkOn.classList.remove("pending");
    checkOn.classList.add("found");
    stepOn.classList.add("done");
  } else {
    checkOn.textContent = "1";
    checkOn.classList.add("pending");
    checkOn.classList.remove("found");
    stepOn.classList.remove("done");
  }

  // Hush Off
  if (off) {
    checkOff.textContent = "✓";
    checkOff.classList.remove("pending");
    checkOff.classList.add("found");
    stepOff.classList.add("done");
  } else {
    checkOff.textContent = "2";
    checkOff.classList.add("pending");
    checkOff.classList.remove("found");
    stepOff.classList.remove("done");
  }

  doneBtn.disabled = !(on && off);
}

function startPolling() {
  if (pollTimer !== null) return;
  pollTimer = window.setInterval(async () => {
    const { on, off } = await checkShortcuts();
    updateChecks(on, off);
    if (on && off && pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }, 2000);
}

function showManualSetup() {
  autoSetup.style.display = "none";
  manualSetup.style.display = "block";
  startPolling();
}

async function init() {
  // Check if shortcuts already exist
  const { on, off } = await checkShortcuts();
  if (on && off) {
    // Already set up — close window
    await invoke("setup_complete");
    return;
  }

  // Show auto-setup spinner briefly, then go to manual
  autoSetup.style.display = "block";

  // Try auto-setup (works on personal devices)
  try {
    const autoOk = await invoke<boolean>("try_auto_setup");
    if (autoOk) {
      const recheck = await checkShortcuts();
      if (recheck.on && recheck.off) {
        await invoke("setup_complete");
        return;
      }
    }
  } catch {
    // Auto-setup failed — show manual
  }

  showManualSetup();
  updateChecks(on, off);
}

openBtn.addEventListener("click", async () => {
  await invoke("open_shortcuts_app");
});

doneBtn.addEventListener("click", async () => {
  manualSetup.style.display = "none";
  allDone.style.display = "block";

  setTimeout(async () => {
    await invoke("setup_complete");
  }, 2000);
});

init();
