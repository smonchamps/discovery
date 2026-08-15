# Sonde de gel de la pompe de messages (PLAN-GELS, décision D3).
#
# Mesure le symptôme « la fenêtre ne répond pas » tel que Windows le
# définit : la pompe de messages du thread principal ne répond plus.
# Lance wind-desktop.exe (release) sur une base donnée — compte factice
# hors ligne, mêmes crochets que les E2E — puis sonde la fenêtre toutes
# les ~100 ms par SendMessageTimeout(WM_NULL) et journalise chaque
# latence au-dessus du seuil. (Latence plafonnée à 5 s par le timeout de
# l'appel : un gel plus long compte 5 000 ms par sonde qui l'observe.)
#
# Budget (PASSATION §3) : AUCUN gel > 150 ms après l'apparition de la
# fenêtre. Sortie non nulle si le budget est dépassé.
#
#   python e2e/sonde-gel.py <base.db> [duree_s=40] [seuil_ms=150]
#
# La base de mesure se place HORS du dépôt (OneDrive fausserait la
# mesure) — PASSATION §7.3. Le constat fondateur (2026-08-15, base
# réelle 251 062 enveloppes, 17 761 aperçus NULL) : 25,2 s de gels
# cumulés sur 40 s AVANT la bascule async des commandes, ~0 après.
#
# L'instrument se vérifie comme le reste (PASSATION §9) : la sortie de
# l'application est DRAINÉE (un tube plein bloquerait le processus fils
# et fabriquerait le gel qu'on mesure — leçon de launch.mjs) et recrachée
# en cas d'échec ; un processus mort arrête la sonde en le disant (sans
# lui, un crash s'imprimerait en faux gels de 0 ms) ; les appels user32
# portent leurs argtypes (un HWND 64 bits tronqué en int C mesurerait
# une fenêtre fantôme).
import collections
import ctypes
import ctypes.wintypes as w
import json
import os
import subprocess
import sys
import threading
import time

if len(sys.argv) < 2:
    print("usage : python e2e/sonde-gel.py <base.db> [duree_s=40] [seuil_ms=150]")
    sys.exit(2)
db = os.path.abspath(sys.argv[1])
duree = float(sys.argv[2]) if len(sys.argv) > 2 else 40.0
seuil = float(sys.argv[3]) if len(sys.argv) > 3 else 150.0
if duree <= 0 or seuil <= 0:
    print(f"duree ({duree}) et seuil ({seuil}) doivent etre positifs — un OK sur 0 s ne prouverait rien")
    sys.exit(2)

user32 = ctypes.windll.user32
SMTO_BLOCK = 0x0001
WM_NULL = 0
# Les argtypes/restype d'abord : sans eux, ctypes tronque un HWND 64 bits
# en int C 32 bits (OverflowError ou extension de signe silencieuse).
user32.SendMessageTimeoutW.argtypes = [
    w.HWND, w.UINT, w.WPARAM, w.LPARAM, w.UINT, w.UINT,
    ctypes.POINTER(ctypes.c_size_t),
]
user32.SendMessageTimeoutW.restype = w.LPARAM
user32.IsWindowVisible.argtypes = [w.HWND]
user32.IsWindowVisible.restype = w.BOOL
user32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]
user32.GetWindowThreadProcessId.restype = w.DWORD

racine = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
exe = os.path.join(racine, "target", "release", "wind-desktop.exe")
if not os.path.exists(exe):
    print(f"binaire absent : {exe} — construire d'abord (cargo build -p wind-desktop --release)")
    sys.exit(2)

# Profil WebView2 dédié À CÔTÉ de la base : jamais celui de la vraie
# application, et hors du dépôt avec elle.
profil = os.path.join(os.path.dirname(db), "webview2-sonde")
os.makedirs(profil, exist_ok=True)

env = dict(os.environ)
env["WIND_DB_PATH"] = db
env["WIND_E2E_ACCOUNT"] = "sonde@exemple.fr"  # jeton invalide : hors ligne garanti
env["WEBVIEW2_USER_DATA_FOLDER"] = profil
# Purge OAuth : la LISTE vit dans isolation-oauth.json — contrat UNIQUE
# partagé avec les lanceurs Node (isolation.mjs) : un fournisseur ajouté
# à un seul endroit couvre tous les lanceurs. Sans la purge, une route
# OAuth ouvrirait un vrai consentement navigateur et suspendrait la sonde.
with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "isolation-oauth.json"), encoding="utf-8") as contrat:
    for cle in json.load(contrat):
        env.pop(cle, None)

t0 = time.perf_counter()
proc = subprocess.Popen([exe], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)

# Drainer la sortie en continu (tube Windows : ~64 Ko — plein, le fils
# bloque en écriture et la sonde mesurerait son propre artefact). On ne
# garde que la fin, recrachée en cas d'échec.
journal = collections.deque(maxlen=200)


def drainer():
    for ligne in proc.stdout:
        journal.append(ligne.decode("utf-8", errors="replace").rstrip())


threading.Thread(target=drainer, daemon=True).start()


def sortie_application():
    return "\n".join(["--- sortie de l'application ---", *journal, "--- fin ---"]) if journal else "(aucune sortie)"


def fenetre_principale(pid):
    """La fenêtre top-level visible du processus."""
    vues = []

    @ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
    def cb(hwnd, _):
        if not user32.IsWindowVisible(hwnd):
            return True
        p = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(p))
        if p.value == pid:
            vues.append(hwnd)
        return True

    user32.EnumWindows(cb, 0)
    return vues[0] if vues else None


def clore(code):
    proc.kill()
    # Attendre la sortie RÉELLE : une relance immédiate reprendrait le
    # profil WebView2 à un processus encore vivant (leçon de closeApp).
    try:
        proc.wait(timeout=15)
    except subprocess.TimeoutExpired:
        print("le processus ne meurt pas en 15 s apres kill()")
    sys.exit(code)


hwnd = None
while hwnd is None and time.perf_counter() - t0 < 30:
    if proc.poll() is not None:
        print(f"ECHEC l'application s'est arretee au demarrage (code {proc.returncode})")
        print(sortie_application())
        clore(1)
    hwnd = fenetre_principale(proc.pid)
    if hwnd is None:
        time.sleep(0.05)
if hwnd is None:
    print("ECHEC fenetre jamais apparue en 30 s")
    print(sortie_application())
    clore(1)

apparue = time.perf_counter() - t0
print(f"fenetre apparue a t+{apparue * 1000:.0f} ms ; sonde {duree:.0f} s, seuil {seuil:.0f} ms")

gels = []
fin = t0 + apparue + duree
while time.perf_counter() < fin:
    if proc.poll() is not None:
        # Un processus mort rend SendMessageTimeout faux immédiatement :
        # sans cette garde, un crash s'imprimerait en faux gels de 0 ms.
        print(f"ECHEC l'application s'est arretee a t+{time.perf_counter() - t0:.2f} s (code {proc.returncode}) — ce n'est pas un gel, c'est un crash")
        print(sortie_application())
        clore(1)
    avant = time.perf_counter()
    res = ctypes.c_size_t()
    ok = user32.SendMessageTimeoutW(hwnd, WM_NULL, 0, 0, SMTO_BLOCK, 5000, ctypes.byref(res))
    latence = (time.perf_counter() - avant) * 1000
    if latence > seuil or not ok:
        gels.append(latence)
        print(
            f"GEL t+{time.perf_counter() - t0:.2f} s : pompe bloquee {latence:.0f} ms"
            f"{' (timeout)' if not ok else ''}",
            flush=True,
        )
    time.sleep(0.1)

cumul = sum(gels) / 1000
if gels:
    print(f"ECHEC : {len(gels)} gel(s) > {seuil:.0f} ms, cumul {cumul:.2f} s sur {duree:.0f} s")
    print(sortie_application())
    clore(1)
print(f"OK : aucun gel > {seuil:.0f} ms sur {duree:.0f} s")
clore(0)
