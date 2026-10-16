param([int]$X, [int]$Y)
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class WFP2 {
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [DllImport("user32.dll")] public static extern int GetClassNameW(IntPtr h, StringBuilder sb, int mx);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int idx);
}
"@
$p = New-Object WFP2+POINT
$p.X = $X; $p.Y = $Y
$h = [WFP2]::WindowFromPoint($p)
$cn = New-Object System.Text.StringBuilder 512
[void][WFP2]::GetClassNameW($h, $cn, 512)
$pid2 = 0
[void][WFP2]::GetWindowThreadProcessId($h, [ref]$pid2)
$ex = [WFP2]::GetWindowLongW($h, -20)
try { $proc = (Get-Process -Id $pid2 -ErrorAction Stop).ProcessName } catch { $proc = "?" }
Write-Output ("hwnd={0} pid={1} proc={2} class='{3}' exstyle=0x{4:X}" -f $h, $pid2, $proc, $cn.ToString(), $ex)

