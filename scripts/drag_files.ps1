param([int]$X1, [int]$Y1, [int]$X2, [int]$Y2)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class DRG {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
}
"@
[void][DRG]::SetCursorPos($X1, $Y1)
Start-Sleep -Milliseconds 400
[DRG]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 200
for ($t = 1; $t -le 14; $t++) {
  $nx = [int]($X1 + ($X2 - $X1) * $t / 14)
  $ny = [int]($Y1 + ($Y2 - $Y1) * $t / 14)
  [void][DRG]::SetCursorPos($nx, $ny)
  Start-Sleep -Milliseconds 70
}
Start-Sleep -Milliseconds 400
[DRG]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero)
Write-Output "dragged $X1,$Y1 -> $X2,$Y2"

