(function () {
  try {
    var key = "streamdubber-theme";
    var stored = localStorage.getItem(key);
    var hour = new Date().getHours();
    var theme = stored === "light" || stored === "dark" ? stored : hour >= 7 && hour < 19 ? "light" : "dark";
    if (theme === "dark") document.documentElement.classList.add("dark");
  } catch (_error) {
    // Storage may be unavailable in hardened browser contexts; light is safe.
  }
})();
