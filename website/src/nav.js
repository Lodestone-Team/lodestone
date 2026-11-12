// The header's Products menu (nav.css). It is a <details>, which opens and
// closes on its own; this closes it when someone clicks elsewhere, presses
// Escape or tabs out of it.

for (const menu of document.querySelectorAll('[data-products-menu]')) {
  const summary = menu.querySelector('summary');

  document.addEventListener('click', (e) => {
    if (menu.open && !menu.contains(e.target)) menu.open = false;
  });

  menu.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && menu.open) {
      menu.open = false;
      summary.focus();
    }
  });

  menu.addEventListener('focusout', (e) => {
    if (menu.open && e.relatedTarget && !menu.contains(e.relatedTarget)) menu.open = false;
  });
}
