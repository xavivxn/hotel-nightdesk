; Start the installed app when this Windows user signs in.
; Tauri's NSIS uninstaller removes the same Run value on a real uninstall
; and preserves it while updating an existing installation.
!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\""
!macroend
