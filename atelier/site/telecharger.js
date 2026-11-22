// Les téléchargements de puppetpaie.fr : le bouton du système du visiteur passe en tête, les deux autres
// en dessous, plus discrets. Sans script, ou sur un téléphone, les trois restent égaux.
(() => {
  const liste = document.getElementById('systemes');
  if (!liste) return;
  const ua = navigator.userAgent || '';
  const plateforme = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || '';
  const s = `${plateforme} ${ua}`;
  if (/Android|iPhone|iPad|iPod/i.test(s)) return;
  const systeme = /Win/i.test(s) ? 'windows' : /Mac/i.test(s) ? 'mac' : /Linux|X11/i.test(s) ? 'linux' : null;
  if (!systeme) return;
  const items = [...liste.querySelectorAll('li')];
  const principal = items.find(li => li.dataset.systeme === systeme);
  if (!principal) return;
  principal.classList.add('principal');
  principal.querySelector('a').textContent = `Télécharger pour ${principal.textContent.trim()}`;
  const autres = document.createElement('p');
  autres.className = 'autres';
  items.filter(li => li !== principal).forEach((li, i) => {
    if (i) autres.append(' · ');
    autres.append(li.querySelector('a'));
    li.remove();
  });
  autres.prepend('ou pour ');
  liste.after(autres);
})();
