param([string]$Match = "*OpenDown*")
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class W {
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
$cb = [W+EnumProc]{
  param($h,$l)
  $len = [W]::GetWindowTextLength($h)
  if ($len -gt 0) {
    $sb = New-Object System.Text.StringBuilder ($len+1)
    [W]::GetWindowText($h, $sb, $len+1) | Out-Null
    $t = $sb.ToString()
    if (($t -like $Match) -and [W]::IsWindowVisible($h)) {
      $r = New-Object W+RECT
      [W]::GetWindowRect($h, [ref]$r) | Out-Null
      [void]$found.Add([pscustomobject]@{H=$h; T=$t; Wd=($r.R-$r.L); Ht=($r.B-$r.T)})
    }
  }
  return $true
}
[W]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
if ($found.Count -eq 0) { Write-Output "NO_WINDOWS"; exit 1 }
$found | ForEach-Object { Write-Output ("CAND ({0}x{1}) '{2}'" -f $_.Wd, $_.Ht, $_.T) }
$best = $found | Sort-Object -Property @{Expression={$_.Wd * $_.Ht}} -Descending | Select-Object -First 1
$r = New-Object W+RECT
[W]::GetWindowRect($best.H, [ref]$r) | Out-Null
$w = $r.R - $r.L; $hh = $r.B - $r.T
$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
$ok = [W]::PrintWindow($best.H, $hdc, 2)
$g.ReleaseHdc($hdc); $g.Dispose()
$bmp.Save((Join-Path $PSScriptRoot "..\_gui_shot.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output ("SAVED " + $w + "x" + $hh + " ok=" + $ok)
