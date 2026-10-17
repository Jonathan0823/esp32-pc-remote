Unicode true
RequestExecutionLevel user

!ifndef APP_VERSION
  !define APP_VERSION "0.1.0"
!endif

Name "ESP32 Media Daemon"
OutFile "target/release/esp32-media-daemon-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\Esp32MediaDaemon"
InstallDirRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esp32MediaDaemon" "InstallLocation"

!include "MUI2.nsh"
!define MUI_ABORTWARNING
!define MUI_ICON "assets\app.ico"
!define MUI_UNICON "assets\app.ico"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File "target\x86_64-pc-windows-msvc\release\esp32-media-daemon.exe"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateDirectory "$SMPROGRAMS\ESP32 Media Daemon"
  CreateShortcut "$SMPROGRAMS\ESP32 Media Daemon\ESP32 Media Daemon.lnk" "$INSTDIR\esp32-media-daemon.exe"
  CreateShortcut "$SMPROGRAMS\ESP32 Media Daemon\Uninstall.lnk" "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esp32MediaDaemon" "DisplayName" "ESP32 Media Daemon"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esp32MediaDaemon" "DisplayIcon" "$\"$INSTDIR\esp32-media-daemon.exe$\",0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esp32MediaDaemon" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esp32MediaDaemon" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esp32MediaDaemon" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esp32MediaDaemon" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esp32MediaDaemon" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$SMPROGRAMS\ESP32 Media Daemon\ESP32 Media Daemon.lnk"
  Delete "$SMPROGRAMS\ESP32 Media Daemon\Uninstall.lnk"
  RMDir "$SMPROGRAMS\ESP32 Media Daemon"
  Delete "$INSTDIR\esp32-media-daemon.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Esp32MediaDaemon"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esp32MediaDaemon"
  ; Keep user configuration, DPAPI secrets, and custom actions under %LOCALAPPDATA%.
SectionEnd
