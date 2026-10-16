; OpenDown installer (NSIS 3.x) - per-user install, no admin required.
Unicode true
!include "MUI2.nsh"

!define APPNAME "OpenDown"
!define VERSION "0.1.0"
!define EXTVERSION "1.0.0"
!define EXTID "manelgpcmclgjpecpncekbbbhjohndhp"
!define PUBLISHER "Akbar R"

Name "${APPNAME} ${VERSION}"
OutFile "..\dist\opendown-setup-v${VERSION}.exe"
InstallDir "$LOCALAPPDATA\Programs\OpenDown"
InstallDirRegKey HKCU "Software\OpenDown" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "0.1.0.0"
VIAddVersionKey "ProductName" "${APPNAME}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "MIT License"

!define MUI_ICON "..\assets\icon.ico"
!define MUI_UNICON "..\assets\icon.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\opendown.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch OpenDown"
!define MUI_FINISHPAGE_SHOWREADME "$INSTDIR\GETTING-STARTED.txt"
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Show setup notes (browser extension info)"
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "OpenDown application" SecApp
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "/oname=opendown.exe" "..\dist\opendown-v${VERSION}-windows-x64.exe"
  File "..\LICENSE"
  File "..\README.md"
  File "GETTING-STARTED.txt"
  SetOutPath "$INSTDIR\extension"
  File "browser-extension-setup.cmd"
  File "..\dist\opendown-extension-v${EXTVERSION}.crx"
  SetOutPath "$INSTDIR\extension\unpacked"
  File /r "..\extension\*.*"
  CreateDirectory "$SMPROGRAMS"
  CreateShortCut "$SMPROGRAMS\OpenDown.lnk" "$INSTDIR\opendown.exe"
  CreateShortCut "$SMPROGRAMS\OpenDown - browser extension setup.lnk" "$INSTDIR\extension\browser-extension-setup.cmd"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\OpenDown" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenDown" "DisplayName" "OpenDown ${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenDown" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenDown" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenDown" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenDown" "DisplayIcon" "$INSTDIR\opendown.exe"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenDown" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenDown" "NoRepair" 1
SectionEnd

Section "Microsoft Edge integration" SecEdge
  WriteRegStr HKCU "Software\Microsoft\Edge\Extensions\${EXTID}" "path" "$INSTDIR\extension\opendown-extension-v${EXTVERSION}.crx"
  WriteRegStr HKCU "Software\Microsoft\Edge\Extensions\${EXTID}" "version" "${EXTVERSION}"
SectionEnd

Section "Google Chrome integration" SecChrome
  WriteRegStr HKCU "Software\Google\Chrome\Extensions\${EXTID}" "path" "$INSTDIR\extension\opendown-extension-v${EXTVERSION}.crx"
  WriteRegStr HKCU "Software\Google\Chrome\Extensions\${EXTID}" "version" "${EXTVERSION}"
SectionEnd

Section /o "Desktop shortcut" SecDesktop
  CreateShortCut "$DESKTOP\OpenDown.lnk" "$INSTDIR\opendown.exe"
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecApp} "Application, CLI, extension files and uninstaller. Required."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecEdge} "Register the OpenDown extension with Microsoft Edge (applies on next browser start, no admin needed)."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecChrome} "Register the OpenDown extension with Google Chrome (applies on next browser start)."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$INSTDIR\opendown.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\GETTING-STARTED.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir /r "$INSTDIR\extension"
  Delete "$SMPROGRAMS\OpenDown.lnk"
  Delete "$SMPROGRAMS\OpenDown - browser extension setup.lnk"
  Delete "$DESKTOP\OpenDown.lnk"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Edge\Extensions\${EXTID}"
  DeleteRegKey HKCU "Software\Google\Chrome\Extensions\${EXTID}"
  DeleteRegKey HKCU "Software\OpenDown"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenDown"
SectionEnd
