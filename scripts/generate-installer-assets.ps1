[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Drawing

$projectRoot = Split-Path -Parent $PSScriptRoot
$outputDirectory = Join-Path $projectRoot "src-tauri/installer"
$iconPath = Join-Path $projectRoot "src-tauri/icons/icon.png"

New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null

function New-BrandBitmap {
  param(
    [int]$Width,
    [int]$Height
  )

  return [System.Drawing.Bitmap]::new(
    $Width,
    $Height,
    [System.Drawing.Imaging.PixelFormat]::Format24bppRgb
  )
}

function New-BrandGraphics {
  param([System.Drawing.Bitmap]$Bitmap)

  $graphics = [System.Drawing.Graphics]::FromImage($Bitmap)
  $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $graphics.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit
  return $graphics
}

function New-RoundedPath {
  param(
    [System.Drawing.RectangleF]$Rectangle,
    [single]$Radius
  )

  $diameter = $Radius * 2
  $path = [System.Drawing.Drawing2D.GraphicsPath]::new()
  $path.AddArc($Rectangle.X, $Rectangle.Y, $diameter, $diameter, 180, 90)
  $path.AddArc($Rectangle.Right - $diameter, $Rectangle.Y, $diameter, $diameter, 270, 90)
  $path.AddArc($Rectangle.Right - $diameter, $Rectangle.Bottom - $diameter, $diameter, $diameter, 0, 90)
  $path.AddArc($Rectangle.X, $Rectangle.Bottom - $diameter, $diameter, $diameter, 90, 90)
  $path.CloseFigure()
  return $path
}

function Fill-RoundedRectangle {
  param(
    [System.Drawing.Graphics]$Graphics,
    [System.Drawing.Brush]$Brush,
    [System.Drawing.RectangleF]$Rectangle,
    [single]$Radius
  )

  $path = New-RoundedPath -Rectangle $Rectangle -Radius $Radius
  try {
    $Graphics.FillPath($Brush, $path)
  } finally {
    $path.Dispose()
  }
}

function Draw-CenteredText {
  param(
    [System.Drawing.Graphics]$Graphics,
    [string]$Text,
    [System.Drawing.Font]$Font,
    [System.Drawing.Brush]$Brush,
    [System.Drawing.RectangleF]$Bounds
  )

  $format = [System.Drawing.StringFormat]::new()
  try {
    $format.Alignment = [System.Drawing.StringAlignment]::Center
    $format.LineAlignment = [System.Drawing.StringAlignment]::Center
    $Graphics.DrawString($Text, $Font, $Brush, $Bounds, $format)
  } finally {
    $format.Dispose()
  }
}

function Draw-RoutePattern {
  param(
    [System.Drawing.Graphics]$Graphics,
    [int]$OffsetX,
    [int]$OffsetY
  )

  $linePen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(69, 69, 69), 1.5)
  $nodePen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(84, 84, 84), 1.5)
  $accentBrush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(30, 196, 181))
  try {
    $Graphics.DrawLine($linePen, $OffsetX + 14, $OffsetY, $OffsetX + 14, $OffsetY + 74)
    $Graphics.DrawLine($linePen, $OffsetX + 14, $OffsetY + 16, $OffsetX + 34, $OffsetY + 16)
    $Graphics.DrawLine($linePen, $OffsetX + 14, $OffsetY + 45, $OffsetX + 34, $OffsetY + 45)
    $Graphics.DrawLine($linePen, $OffsetX + 14, $OffsetY + 74, $OffsetX + 34, $OffsetY + 74)

    foreach ($y in @(0, 29, 58)) {
      $Graphics.DrawEllipse($nodePen, $OffsetX + 8, $OffsetY + $y + 10, 12, 12)
    }
    $Graphics.FillEllipse($accentBrush, $OffsetX + 10, $OffsetY + 12, 8, 8)
  } finally {
    $linePen.Dispose()
    $nodePen.Dispose()
    $accentBrush.Dispose()
  }
}

function Save-NsisSidebar {
  param(
    [System.Drawing.Image]$Icon,
    [string]$Path
  )

  $bitmap = New-BrandBitmap -Width 164 -Height 314
  $graphics = New-BrandGraphics -Bitmap $bitmap
  $background = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(42, 42, 42))
  $panel = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(50, 50, 50))
  $primary = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(245, 245, 245))
  $secondary = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(160, 160, 160))
  $accent = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(30, 196, 181))
  $titleFont = [System.Drawing.Font]::new("Segoe UI Semibold", 17, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
  $captionFont = [System.Drawing.Font]::new("Segoe UI", 8, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
  $itemFont = [System.Drawing.Font]::new("Segoe UI", 9, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
  try {
    $graphics.FillRectangle($background, 0, 0, 164, 314)
    $graphics.FillRectangle($accent, 0, 0, 4, 314)
    $graphics.DrawImage($Icon, 44, 24, 76, 76)
    Draw-CenteredText -Graphics $graphics -Text "CCHub" -Font $titleFont -Brush $primary -Bounds ([System.Drawing.RectangleF]::new(8, 104, 148, 28))
    Draw-CenteredText -Graphics $graphics -Text "CONFIG WORKSPACE" -Font $captionFont -Brush $secondary -Bounds ([System.Drawing.RectangleF]::new(8, 132, 148, 18))

    Fill-RoundedRectangle -Graphics $graphics -Brush $panel -Rectangle ([System.Drawing.RectangleF]::new(22, 169, 120, 102)) -Radius 7
    Draw-RoutePattern -Graphics $graphics -OffsetX 28 -OffsetY 178
    $graphics.DrawString("Install", $itemFont, $primary, 66, 185)
    $graphics.DrawString("Configure", $itemFont, $secondary, 66, 214)
    $graphics.DrawString("Ready", $itemFont, $secondary, 66, 243)

    Draw-CenteredText -Graphics $graphics -Text "SWITCH WITH CONFIDENCE" -Font $captionFont -Brush $secondary -Bounds ([System.Drawing.RectangleF]::new(8, 284, 148, 18))
    $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Bmp)
  } finally {
    $itemFont.Dispose()
    $captionFont.Dispose()
    $titleFont.Dispose()
    $accent.Dispose()
    $secondary.Dispose()
    $primary.Dispose()
    $panel.Dispose()
    $background.Dispose()
    $graphics.Dispose()
    $bitmap.Dispose()
  }
}

function Draw-HeaderArtwork {
  param(
    [System.Drawing.Graphics]$Graphics,
    [System.Drawing.Image]$Icon,
    [int]$Width,
    [int]$Height,
    [bool]$IsUninstall
  )

  $background = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(248, 248, 248))
  $accent = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(30, 196, 181))
  $linePen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(218, 218, 218), 1)
  try {
    $Graphics.FillRectangle($background, 0, 0, $Width, $Height)
    $Graphics.FillRectangle($accent, 0, 0, 4, $Height)

    $iconSize = [Math]::Min(44, $Height - 12)
    $iconX = $Width - $iconSize - 9
    $iconY = [Math]::Floor(($Height - $iconSize) / 2)
    $Graphics.DrawLine($linePen, $iconX - 43, 20, $iconX - 12, 20)
    $Graphics.DrawLine($linePen, $iconX - 29, 29, $iconX - 12, 29)
    $graphics.DrawLine($linePen, $iconX - 37, 38, $iconX - 12, 38)

    if ($IsUninstall) {
      $Graphics.FillRectangle($accent, $iconX - 7, $Height - 9, 4, 4)
    } else {
      $Graphics.FillRectangle($accent, $iconX - 7, 5, 4, 4)
    }
    $Graphics.DrawImage($Icon, $iconX, $iconY, $iconSize, $iconSize)
  } finally {
    $linePen.Dispose()
    $accent.Dispose()
    $background.Dispose()
  }
}

function Save-Header {
  param(
    [System.Drawing.Image]$Icon,
    [string]$Path,
    [int]$Width,
    [int]$Height,
    [bool]$IsUninstall = $false
  )

  $bitmap = New-BrandBitmap -Width $Width -Height $Height
  $graphics = New-BrandGraphics -Bitmap $bitmap
  try {
    Draw-HeaderArtwork -Graphics $graphics -Icon $Icon -Width $Width -Height $Height -IsUninstall $IsUninstall
    $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Bmp)
  } finally {
    $graphics.Dispose()
    $bitmap.Dispose()
  }
}

function Save-WixDialog {
  param(
    [System.Drawing.Image]$Icon,
    [string]$Path
  )

  $bitmap = New-BrandBitmap -Width 493 -Height 312
  $graphics = New-BrandGraphics -Bitmap $bitmap
  $light = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(248, 248, 248))
  $dark = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(42, 42, 42))
  $panel = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(50, 50, 50))
  $accent = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(30, 196, 181))
  $primary = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(245, 245, 245))
  $secondary = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(160, 160, 160))
  $titleFont = [System.Drawing.Font]::new("Segoe UI Semibold", 17, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
  $captionFont = [System.Drawing.Font]::new("Segoe UI", 8, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
  $itemFont = [System.Drawing.Font]::new("Segoe UI", 9, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
  try {
    $graphics.FillRectangle($light, 0, 0, 493, 312)
    $graphics.FillRectangle($dark, 0, 0, 164, 312)
    $graphics.FillRectangle($accent, 0, 0, 4, 312)
    $graphics.DrawImage($Icon, 44, 25, 76, 76)
    Draw-CenteredText -Graphics $graphics -Text "CCHub" -Font $titleFont -Brush $primary -Bounds ([System.Drawing.RectangleF]::new(8, 105, 148, 28))
    Draw-CenteredText -Graphics $graphics -Text "CONFIG WORKSPACE" -Font $captionFont -Brush $secondary -Bounds ([System.Drawing.RectangleF]::new(8, 133, 148, 18))
    Fill-RoundedRectangle -Graphics $graphics -Brush $panel -Rectangle ([System.Drawing.RectangleF]::new(22, 171, 120, 102)) -Radius 7
    Draw-RoutePattern -Graphics $graphics -OffsetX 28 -OffsetY 180
    $graphics.DrawString("Install", $itemFont, $primary, 66, 187)
    $graphics.DrawString("Configure", $itemFont, $secondary, 66, 216)
    $graphics.DrawString("Ready", $itemFont, $secondary, 66, 245)
    Draw-CenteredText -Graphics $graphics -Text "SWITCH WITH CONFIDENCE" -Font $captionFont -Brush $secondary -Bounds ([System.Drawing.RectangleF]::new(8, 285, 148, 18))
    $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Bmp)
  } finally {
    $itemFont.Dispose()
    $captionFont.Dispose()
    $titleFont.Dispose()
    $secondary.Dispose()
    $primary.Dispose()
    $accent.Dispose()
    $panel.Dispose()
    $dark.Dispose()
    $light.Dispose()
    $graphics.Dispose()
    $bitmap.Dispose()
  }
}

$icon = [System.Drawing.Image]::FromFile($iconPath)
try {
  Save-NsisSidebar -Icon $icon -Path (Join-Path $outputDirectory "nsis-sidebar.bmp")
  Save-Header -Icon $icon -Path (Join-Path $outputDirectory "nsis-header.bmp") -Width 150 -Height 57
  Save-Header -Icon $icon -Path (Join-Path $outputDirectory "nsis-uninstall-header.bmp") -Width 150 -Height 57 -IsUninstall $true
  Save-Header -Icon $icon -Path (Join-Path $outputDirectory "wix-banner.bmp") -Width 493 -Height 58
  Save-WixDialog -Icon $icon -Path (Join-Path $outputDirectory "wix-dialog.bmp")
} finally {
  $icon.Dispose()
}

Write-Host "Generated Windows installer artwork in $outputDirectory"
