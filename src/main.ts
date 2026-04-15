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

// Copy buttons for shortcut names
const copyIconSvg = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>';
const checkIconSvg = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>';

document.querySelectorAll(".btn-copy").forEach((btn) => {
  btn.addEventListener("click", () => {
    const text = (btn as HTMLElement).dataset.copy;
    if (text) {
      navigator.clipboard.writeText(text).then(() => {
        btn.innerHTML = checkIconSvg;
        btn.classList.add("copied");
        setTimeout(() => { btn.innerHTML = copyIconSvg; btn.classList.remove("copied"); }, 1500);
      });
    }
  });
});

doneBtn.addEventListener("click", async () => {
  manualSetup.style.display = "none";
  allDone.style.display = "block";

  setTimeout(async () => {
    await invoke("setup_complete");
  }, 2000);
});

init();
