Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public class W4 {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public struct RECT { public int L,T,R,B; }
}
"@
$found = New-Object System.Collections.ArrayList
$cb = [W4+EnumProc]{
  param($h,$l)
  $len = [W4]::GetWindowTextLength($h)
  if ($len -gt 0) {
    $sb = New-Object System.Text.StringBuilder ($len+1)
    [W4]::GetWindowText($h, $sb, $len+1) | Out-Null
    $t = $sb.ToString()
    if ($t -like "*opendown*") {
      $pid2 = 0
      [void][W4]::GetWindowThreadProcessId($h, [ref]$pid2)
      $r = New-Object W4+RECT
      [void][W4]::GetWindowRect($h, [ref]$r)
      [void]$found.Add(("H={0} PID={1} VIS={2} POS=({3},{4}) '{5}'" -f $h, $pid2, [W4]::IsWindowVisible($h), $r.L, $r.T, $t))
    }
  }
  return $true
}
[W4]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
if ($found.Count -eq 0) { Write-Output "NONE" } else { $found | ForEach-Object { Write-Output $_ } }
