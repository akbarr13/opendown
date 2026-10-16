param([string]$Match = "*OpenDown*", [string]$Points = "0,0")
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class WP {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint nFlags);
  public struct RECT { public int L,T,R,B; }
}
"@
$found = New-Object System.Collections.ArrayList
$cb = [WP+EnumProc]{
  param($h,$l)
  $len = [WP]::GetWindowTextLength($h)
  if ($len -gt 0) {
    $sb = New-Object System.Text.StringBuilder ($len+1)
    [WP]::GetWindowText($h, $sb, $len+1) | Out-Null
    $t = $sb.ToString()
    if (($t -like $Match) -and [WP]::IsWindowVisible($h)) {
      $r = New-Object WP+RECT
      [WP]::GetWindowRect($h, [ref]$r) | Out-Null
      [void]$found.Add([pscustomobject]@{H=$h; T=$t; Wd=($r.R-$r.L); Ht=($r.B-$r.T)})
    }
  }
  return $true
}
[WP]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
if ($found.Count -eq 0) { Write-Output "NO_WINDOWS"; exit 1 }
$best = $found | Sort-Object -Property @{Expression={$_.Wd * $_.Ht}} -Descending | Select-Object -First 1
$r = New-Object WP+RECT
[WP]::GetWindowRect($best.H, [ref]$r) | Out-Null
$w = $r.R - $r.L; $hh = $r.B - $r.T
$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
[void][WP]::PrintWindow($best.H, $hdc, 2)
$g.ReleaseHdc($hdc); $g.Dispose()
foreach ($pt in $Points.Split(";")) {
  $xy = $pt.Split(",")
  $px = [int]$xy[0]; $py = [int]$xy[1]
  if ($px -ge 0 -and $px -lt $w -and $py -ge 0 -and $py -lt $hh) {
    $c = $bmp.GetPixel($px, $py)
    Write-Output ("PIXEL " + $px + "," + $py + " = R" + $c.R + " G" + $c.G + " B" + $c.B)
  }
}
$bmp.Dispose()
Write-Output "PX_DONE"

