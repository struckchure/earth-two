; Generated paths are supplied by package.py. Install for the current user.
Unicode true
!include "MUI2.nsh"
Name "Earth Two"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\Earth Two"
InstallDirRegKey HKCU "Software\Earth Two" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Earth Two"
VIAddVersionKey "FileDescription" "Earth Two installer"
VIAddVersionKey "FileVersion" "${VERSION}"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Earth Two"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File /r "${PAYLOAD}\*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\Earth Two"
  CreateShortcut "$SMPROGRAMS\Earth Two\Earth Two.lnk" "$INSTDIR\earth-two.exe"
  CreateShortcut "$SMPROGRAMS\Earth Two\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Earth Two" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Earth Two" "DisplayName" "Earth Two"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Earth Two" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Earth Two" "Publisher" "Earth Two"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Earth Two" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Earth Two" "DisplayIcon" "$INSTDIR\earth-two.exe"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Earth Two" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Earth Two" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$SMPROGRAMS\Earth Two\Earth Two.lnk"
  Delete "$SMPROGRAMS\Earth Two\Uninstall.lnk"
  RMDir "$SMPROGRAMS\Earth Two"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Earth Two"
  DeleteRegKey HKCU "Software\Earth Two"
  ; Installed runtime files only; player keys live separately in $HOME/earth-two.
  RMDir /r "$INSTDIR\assets"
  Delete "$INSTDIR\earth-two.exe"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
