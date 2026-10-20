#!/bin/bash
# Richtet den OLAT-Abgleich auf einem Mac ein. Start: den Befehl aus der App (Einrichten bzw. Konto → Mac) ins Terminal einfügen;
# er setzt LH_REPO und LH_TOKEN (beim Lernhelfer-Server auch LH_API) und LH_APP, die Adresse der App.
# Braucht kein Administratorpasswort; alles landet in ~/Library/Lernhelfer, der automatische Abgleich ist ein launchd-Job.
# Nochmal starten = aktualisieren (Python und Kurse bleiben, holen.py und Zugang werden erneuert).
# Läuft mit dem bash 3.2 von macOS: keine Arrays, kein ${x,,}, kein mapfile.
set -euo pipefail

APP="${LH_APP:-https://xnik243.github.io/lernhelfer/}"
case "$APP" in */) ;; *) APP="$APP/" ;; esac
LH="${LH_ORT:-$HOME/Library/Lernhelfer}"  # LH_ORT: nur zum Testen
LABEL="at.lernhelfer.olat${LH_ORT:+.test}"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
PY="$LH/python/bin/python3"
# Python ohne Installer und ohne Admin-Rechte: python-build-standalone (wie uv), Fassung und Prüfsummen fest
PY_URL='https://github.com/astral-sh/python-build-standalone/releases/download/20260924/cpython-3.14.7%2B20260924-ARCH-apple-darwin-install_only.tar.gz'
PY_SHA_aarch64='d3da099bb2bdd57e2f5ff8496cb9827f7d92eee332b09f8dc93706dabfc51a96'
PY_SHA_x86_64='11ff6db91dadbf8544d437e617586ee5184382ce95e4a44b7a64864db48cfe87'

schritt() { printf '\n \033[36m%s/5  %s\033[0m\n' "$1" "$2"; }
info() { printf '      %s\n' "$1"; }
fehler() { printf '\n \033[31mFehler: %s\033[0m\n\n' "$1" >&2; exit 1; }
trap 'printf "\n \033[31mEtwas hat nicht geklappt (Zeile %s). Die Meldung oben und die Anleitung im Lernhelfer helfen weiter.\033[0m\n\n" "$LINENO" >&2' ERR

[ "$(uname -s)" = Darwin ] || fehler 'Das ist das Mac-Programm. Für Windows gibt es in der App die Datei „Lernhelfer-PC-einrichten“.'
case "${LH_REPO:-}${LH_TOKEN:-}" in *undefined*|'') fehler 'Bitte den ganzen Befehl aus der App kopieren (Einrichten bzw. Konto → Mac) und hier einfügen.' ;; esac
[ -n "${LH_REPO:-}" ] && [ -n "${LH_TOKEN:-}" ] || fehler 'Bitte den ganzen Befehl aus der App kopieren (Einrichten bzw. Konto → Mac) und hier einfügen.'
MACOS="$(sw_vers -productVersion)"
[ "${MACOS%%.*}" -ge 12 ] || fehler "Der Abgleich braucht macOS 12 (Monterey) oder neuer, dieser Mac hat $MACOS."
ARCH="$(uname -m)"
if [ "$(sysctl -in sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then ARCH=arm64; fi  # Terminal unter Rosetta: trotzdem die Apple-Silicon-Fassung
case "$ARCH" in arm64) ARCH=aarch64; SHA="$PY_SHA_aarch64" ;; x86_64) SHA="$PY_SHA_x86_64" ;; *) fehler "Unbekannter Prozessor: $ARCH" ;; esac

printf '\n \033[33mLERNHELFER · OLAT-Abgleich für den Mac einrichten\033[0m\n'
echo ' Dauert beim ersten Mal 5 bis 15 Minuten. Fenster bitte offen lassen.'
mkdir -p "$LH"

schritt 1 'Python laden (einmalig, etwa 27 MB)'
if "$PY" -c 'import sys; assert sys.version_info >= (3, 12)' 2>/dev/null; then info 'schon da'; else
  rm -rf "$LH/python" "$LH/python.tgz"
  curl -fL --retry 3 --progress-bar -o "$LH/python.tgz" "${PY_URL/ARCH/$ARCH}" || fehler 'Python ließ sich nicht laden. Internetverbindung prüfen und nochmal starten.'
  [ "$(shasum -a 256 "$LH/python.tgz" | cut -d' ' -f1)" = "$SHA" ] || { rm -f "$LH/python.tgz"; fehler 'Die Python-Datei kam nicht vollständig an. Bitte nochmal starten.'; }
  tar -xzf "$LH/python.tgz" -C "$LH"
  rm -f "$LH/python.tgz"
  "$PY" -V >/dev/null || fehler 'Python ließ sich nicht starten.'
fi

schritt 2 'Browser-Steuerung und PDF-Leser installieren'
"$PY" -m pip install --quiet --upgrade --disable-pip-version-check --only-binary=:all: playwright pypdf truststore \
  || fehler 'Die Pakete ließen sich nicht installieren. Internetverbindung prüfen und nochmal starten.'
if [ -d '/Applications/Google Chrome.app' ]; then info 'Browser für OLAT: Google Chrome'
else
  info 'Google Chrome ist nicht da, darum lädt der Lernhelfer sein eigenes Chromium (einmalig, etwa 250 MB).'
  "$PY" -m playwright install chromium || fehler 'Chromium ließ sich nicht laden. Internetverbindung prüfen und nochmal starten.'
fi

schritt 3 'Lernhelfer-Abgleich laden und verbinden'
curl -fsSL --retry 3 -o "$LH/holen.py.neu" "${APP}holen.py" || fehler 'holen.py ließ sich nicht laden. Internetverbindung prüfen und nochmal starten.'
"$PY" -c 'import sys; compile(open(sys.argv[1], "rb").read(), "holen.py", "exec")' "$LH/holen.py.neu" || fehler 'holen.py kam nicht vollständig an. Bitte nochmal starten.'
mv -f "$LH/holen.py.neu" "$LH/holen.py"
( umask 077  # der Zugang enthält den Speicher-Schlüssel: nur für dich lesbar
  LH_APP="$APP" "$PY" -c 'import json, os, sys
json.dump({"repo": os.environ["LH_REPO"], "token": os.environ["LH_TOKEN"], "api": os.environ.get("LH_API") or None, "app": os.environ["LH_APP"]},
          open(sys.argv[1], "w", encoding="utf-8"))' "$LH/zugang.json" )
# Doppelklick-Datei für den Abgleich von Hand (neu einloggen, sofort abgleichen); selbst erzeugt, darum ohne Gatekeeper-Warnung
cat > "$LH/Lernhelfer OLAT.command" <<EOF
#!/bin/bash
# OLAT-Unterlagen sofort abgleichen oder neu bei OLAT einloggen
cd "$LH" && ./python/bin/python3 holen.py "\$@"
echo
read -r -p 'Fertig. Mit Enter schließen.' _ || true
EOF
chmod +x "$LH/Lernhelfer OLAT.command"
VERKNUEPFUNG="$LH/Lernhelfer OLAT.command"
if [ -z "${LH_ORT:-}" ]; then  # fragt macOS, ob das Terminal auf den Schreibtisch darf: „Erlauben“; sonst bleibt sie im Lernhelfer-Ordner
  if cp -f "$LH/Lernhelfer OLAT.command" "$HOME/Desktop/" 2>/dev/null; then VERKNUEPFUNG="$HOME/Desktop/Lernhelfer OLAT.command"
  else info 'Auf den Schreibtisch durfte die Verknüpfung nicht, sie liegt im Lernhelfer-Ordner (öffnet sich gleich im Finder).'; open -R "$VERKNUEPFUNG" || true; fi
fi

schritt 4 "Erster Abgleich: Gleich öffnet sich $( [ -d '/Applications/Google Chrome.app' ] && echo Chrome || echo Chromium ). Dort bei OLAT einloggen."
info 'Danach sucht der Lernhelfer deine Kurse und lädt die Unterlagen. Bewertete Onlinetests'
info 'startet er nie, er liest nur ihre Startseite. Gespeichert oder abgeschickt wird nie etwas.'
ERSTER=0
( cd "$LH" && "$PY" holen.py ) || ERSTER=$?

schritt 5 'Automatischen Abgleich einrichten (launchd)'
mkdir -p "$HOME/Library/LaunchAgents"
"$PY" - "$PLIST" "$LABEL" "$LH" <<'EOF'
import plistlib, sys
ziel, label, lh = sys.argv[1:4]
with open(ziel, "wb") as f:  # beim Anmelden und alle 30 Minuten; verpasste Läufe holt launchd nach dem Aufwachen nach
    plistlib.dump({"Label": label, "ProgramArguments": [lh + "/python/bin/python3", lh + "/holen.py", "--auto"], "WorkingDirectory": lh,
                   "RunAtLoad": True, "StartInterval": 1800, "StandardOutPath": lh + "/launchd.log", "StandardErrorPath": lh + "/launchd.log",
                   "EnvironmentVariables": {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"}}, f)
EOF
plutil -lint "$PLIST" >/dev/null || fehler 'Die launchd-Datei ist kaputt.'
DOMAIN="gui/$(id -u)"
launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
GELADEN=''
for _ in 1 2 3; do  # direkt nach bootout meldet launchd manchmal noch „Input/output error“
  if launchctl bootstrap "$DOMAIN" "$PLIST" 2>/dev/null; then GELADEN=1; break; fi
  sleep 2
done
[ -n "$GELADEN" ] || launchctl load -w "$PLIST" 2>/dev/null || fehler 'Der automatische Abgleich ließ sich nicht einschalten. Bitte nochmal starten.'
info 'läuft ab jetzt beim Anmelden und alle 30 Minuten, solange der Mac wach ist'
info 'Meldet macOS „Hintergrundobjekt hinzugefügt“: Das ist der Abgleich, bitte eingeschaltet lassen.'

[ "$ERSTER" = 0 ] || fehler "Der erste Abgleich ist nicht durchgelaufen. Nochmal versuchen: „Lernhelfer OLAT“ doppelklicken ($VERKNUEPFUNG)."
printf '\n \033[32mFertig. Die App zeigt jetzt deine Fächer.\033[0m\n'
echo ' Läuft die OLAT-Anmeldung einmal ab, öffnet sich das Login-Fenster von selbst, oder du doppelklickst „Lernhelfer OLAT“.'
echo
