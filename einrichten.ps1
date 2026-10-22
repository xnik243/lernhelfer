# Richtet den OLAT-Abgleich auf einem Windows-PC ein. Gestartet von „Lernhelfer-PC-einrichten.cmd“ aus der App,
# die LH_REPO, LH_TOKEN und LH_APP setzt (beim Lernhelfer-Server auch LH_API). Braucht keine Administratorrechte; alles landet in %LOCALAPPDATA%\Lernhelfer.
# Nochmal starten = aktualisieren (Python und Kurse bleiben, holen.py und Zugang werden erneuert).
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'  # sonst lädt Invoke-WebRequest in Windows PowerShell quälend langsam
[Console]::OutputEncoding = [Text.Encoding]::UTF8
[Net.ServicePointManager]::SecurityProtocol = 'Tls12'
$App = if ($env:LH_APP) { $env:LH_APP.TrimEnd('/') + '/' } else { 'https://xnik243.github.io/lernhelfer/' }  # Server-Version: holen.py von dort
$PythonZip = 'https://www.python.org/ftp/python/3.14.8/python-3.14.8-embed-amd64.zip'
$PythonSha = 'a93abe456ab01bd96d7a085b3cdb6566b3063f4241360d114142fbdb07f0a310'  # wie auf python.org angegeben
$LH = if ($env:LH_ORT) { $env:LH_ORT } else { Join-Path $env:LOCALAPPDATA 'Lernhelfer' }  # LH_ORT: nur zum Testen
$Aufgabe = if ($env:LH_ORT) { 'Lernhelfer OLAT-Abgleich (Test)' } else { 'Lernhelfer OLAT-Abgleich' }
$OhneBom = New-Object Text.UTF8Encoding $false

function Schritt($nr, $text) { Write-Host ''; Write-Host " $nr/5  $text" -ForegroundColor Cyan }

try {
  if (-not $env:LH_REPO -or -not $env:LH_TOKEN -or $env:LH_REPO -eq 'undefined' -or $env:LH_TOKEN -eq 'undefined') { throw 'Bitte die Datei „Lernhelfer-PC-einrichten.cmd“ aus der App starten (Einrichten → PC).' }
  Write-Host ''
  Write-Host ' LERNHELFER · OLAT-Abgleich einrichten' -ForegroundColor Yellow
  Write-Host ' Dauert beim ersten Mal 5 bis 15 Minuten. Fenster bitte offen lassen.'
  New-Item -ItemType Directory -Force $LH | Out-Null
  $Py = Join-Path $LH 'python\python.exe'

  Schritt 1 'Python laden (einmalig, etwa 12 MB)'
  if (Test-Path $Py) { Write-Host '      schon da' } else {
    $zip = Join-Path $LH 'python.zip'
    Invoke-WebRequest -UseBasicParsing $PythonZip -OutFile $zip
    if ((Get-FileHash $zip -Algorithm SHA256).Hash -ne $PythonSha) { Remove-Item $zip; throw 'Die Python-Datei kam nicht vollständig an. Bitte nochmal starten.' }
    Expand-Archive $zip (Join-Path $LH 'python') -Force
    Remove-Item $zip
    $pth = Get-ChildItem (Join-Path $LH 'python') -Filter 'python*._pth' | Select-Object -First 1
    (Get-Content $pth.FullName) -replace '^#\s*import site', 'import site' | Set-Content $pth.FullName -Encoding Ascii  # damit pip-Pakete gefunden werden
    Invoke-WebRequest -UseBasicParsing 'https://bootstrap.pypa.io/get-pip.py' -OutFile (Join-Path $LH 'get-pip.py')
    & $Py (Join-Path $LH 'get-pip.py') --quiet --no-warn-script-location
    if ($LASTEXITCODE) { throw 'pip ließ sich nicht installieren.' }
    Remove-Item (Join-Path $LH 'get-pip.py')
  }

  Schritt 2 'Browser-Steuerung und PDF-Leser installieren'
  & $Py -m pip install --quiet --upgrade --no-warn-script-location playwright pypdf
  if ($LASTEXITCODE) { throw 'Die Pakete ließen sich nicht installieren. Internetverbindung prüfen und nochmal starten.' }
  $chrome = @("$env:ProgramFiles\Google\Chrome\Application\chrome.exe", "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe",
              "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe") | Where-Object { Test-Path $_ }
  Write-Host ('      Browser für OLAT: ' + $(if ($chrome) { 'Google Chrome' } else { 'Microsoft Edge (Chrome nicht gefunden)' }))

  Schritt 3 'Lernhelfer-Abgleich laden und verbinden'
  Invoke-WebRequest -UseBasicParsing ($App + 'holen.py') -OutFile (Join-Path $LH 'holen.py')
  [IO.File]::WriteAllText((Join-Path $LH 'zugang.json'), (@{ repo = $env:LH_REPO; token = $env:LH_TOKEN; api = $env:LH_API; app = $App } | ConvertTo-Json), $OhneBom)
  # Doppelklick-Datei für den Abgleich von Hand (neu einloggen, sofort abgleichen), dazu eine Verknüpfung auf dem Desktop
  [IO.File]::WriteAllText((Join-Path $LH 'Abgleich starten.cmd'), "@echo off`r`ncd /d `"%~dp0`"`r`ntitle Lernhelfer OLAT-Abgleich`r`npython\python.exe holen.py %*`r`necho.`r`npause`r`n", $OhneBom)
  if (-not $env:LH_ORT) {
    $lnk = (New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path ([Environment]::GetFolderPath('Desktop')) 'Lernhelfer OLAT.lnk'))
    $lnk.TargetPath = Join-Path $LH 'Abgleich starten.cmd'; $lnk.WorkingDirectory = $LH
    $lnk.Description = 'OLAT-Unterlagen sofort abgleichen oder neu bei OLAT einloggen'; $lnk.Save()
  }

  Schritt 4 'Automatischen Abgleich einrichten (Aufgabenplanung)'
  $aktion = New-ScheduledTaskAction -Execute (Join-Path $LH 'python\pythonw.exe') -Argument "`"$LH\holen.py`" --auto" -WorkingDirectory $LH
  $ausloeser = @(
    (New-ScheduledTaskTrigger -Once -At (Get-Date).Date.AddHours(6) -RepetitionInterval (New-TimeSpan -Minutes 30) -RepetitionDuration (New-TimeSpan -Days 3650)),
    (New-ScheduledTaskTrigger -AtLogOn -User "$env:USERDOMAIN\$env:USERNAME"))
  $einstellung = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit (New-TimeSpan -Hours 1) -MultipleInstances IgnoreNew
  Register-ScheduledTask -TaskName $Aufgabe -Action $aktion -Trigger $ausloeser -Settings $einstellung -Force `
    -Description "Prüft alle 30 min, ob ein Abgleich fällig ist; OLAT sieht höchstens alle 12 h einen Abgleich, nur tagsüber. Log: $LH\holen.log" | Out-Null
  Write-Host '      läuft ab jetzt alle 30 Minuten, solange der PC an ist'

  Schritt 5 'Erster Abgleich: Gleich öffnet sich Chrome. Dort bei OLAT einloggen.'
  Write-Host '      Danach sucht der Lernhelfer deine Kurse und lädt die Unterlagen. Bewertete Onlinetests'
  Write-Host '      startet er nie, er liest nur ihre Startseite. Gespeichert oder abgeschickt wird nie etwas.'
  Push-Location $LH
  & $Py holen.py
  $fehler = $LASTEXITCODE
  Pop-Location
  if ($fehler) { throw 'Der erste Abgleich ist nicht durchgelaufen. Nochmal versuchen: auf dem Desktop „Lernhelfer OLAT“ doppelklicken.' }
  Write-Host ''
  Write-Host ' Fertig. Die App zeigt jetzt deine Fächer.' -ForegroundColor Green
  Write-Host ' Läuft die OLAT-Anmeldung einmal ab, öffnet sich Chrome von selbst oder du doppelklickst „Lernhelfer OLAT“.'
  exit 0
} catch {
  Write-Host ''
  Write-Host (' Fehler: ' + $_.Exception.Message) -ForegroundColor Red
  exit 1
}
