(function () {
  try {
    var key = "streamdubber-theme";
    var stored = localStorage.getItem(key);
    var prefersDark = window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches;
    var theme = stored === "light" || stored === "dark" ? stored : prefersDark ? "dark" : "light";
    if (theme === "dark") document.documentElement.classList.add("dark");
  } catch (_error) {
    // Storage or matchMedia may be unavailable in hardened browser contexts; light is safe.
  }
})();
