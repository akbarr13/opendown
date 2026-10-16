param([int]$X, [int]$Y, [long]$Hwnd = 0)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class CLK3 {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
}
"@
if ($Hwnd -ne 0) {
  $h = [IntPtr]::new($Hwnd)
  [void][CLK3]::ShowWindow($h, 9)          # SW_RESTORE
  [void][CLK3]::SetWindowPos($h, [IntPtr]::new(-1), 0,0,0,0, 0x0003)  # HWND_TOP, NOSIZE|NOMOVE
  [void][CLK3]::SetForegroundWindow($h)
  Start-Sleep -Milliseconds 500
}
[void][CLK3]::SetCursorPos($X - 50, $Y - 50)
Start-Sleep -Milliseconds 150
[void][CLK3]::SetCursorPos($X, $Y)
Start-Sleep -Milliseconds 300
[CLK3]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 300
[CLK3]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero)
Write-Output "clicked $X $Y on $Hwnd"

