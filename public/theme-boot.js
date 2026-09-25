// Applies the saved theme before the first paint, so opening in dark mode
// never flashes white. Mirrors readPreference/resolveTheme in src/lib/theme.ts.
(function () {
  var preference = 'system';
  try {
    var saved = window.localStorage.getItem('ourfault.theme');
    if (saved === 'light' || saved === 'dark' || saved === 'system') preference = saved;
  } catch (error) {
    // Storage unavailable: follow the system.
  }
  var systemDark =
    typeof window.matchMedia === 'function' && window.matchMedia('(prefers-color-scheme: dark)').matches;
  var dark = preference === 'dark' || (preference === 'system' && systemDark);
  document.documentElement.setAttribute('data-theme', dark ? 'dark' : 'light');
})();
