// Première requête du frontend : sert aussi de mesure « fenêtre utilisable ».
window.__TAURI__.core.invoke('startup_report').then((report) => {
  document.getElementById('status').textContent = report;
});
