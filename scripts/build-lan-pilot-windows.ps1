$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..'))

if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
  throw 'Este piloto genera un instalador Windows x64. Confirmá la arquitectura antes de compilar.'
}
if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
  throw 'Falta Node.js/npm. Instalá los requisitos de Tauri y volvé a ejecutar este script.'
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw 'Falta Rust/cargo. Instalá los requisitos de Tauri y volvé a ejecutar este script.'
}

$env:NIGHTDESK_PILOT_BUILD = '1'
npm ci
if ($LASTEXITCODE -ne 0) { throw 'npm ci falló' }
npm run tauri -- build --debug --bundles nsis --no-sign --config src-tauri/tauri.pilot.auto.conf.json
if ($LASTEXITCODE -ne 0) { throw 'La compilación del instalador falló' }

Write-Host 'Instalador de ensayo:'
Get-ChildItem 'src-tauri/target/debug/bundle/nsis/Nightdesk LAN Pilot Auto*setup.exe' | Select-Object FullName, Length
