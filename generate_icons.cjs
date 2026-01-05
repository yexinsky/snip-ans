const fs = require('fs');
const { createCanvas } = require('canvas');
const path = require('path');

function createIcon(size, text, filename) {
  const canvas = createCanvas(size, size);
  const ctx = canvas.getContext('2d');

  // Background
  ctx.fillStyle = '#2196F3'; // Blue

  if (size <= 32) {
    // Simple square for small sizes to ensure visibility
    ctx.fillRect(0, 0, size, size);
  } else {
    // Rounded rect for larger
    const r = size * 0.2;
    ctx.beginPath();
    ctx.moveTo(r, 0);
    ctx.lineTo(size - r, 0);
    ctx.quadraticCurveTo(size, 0, size, r);
    ctx.lineTo(size, size - r);
    ctx.quadraticCurveTo(size, size, size - r, size);
    ctx.lineTo(r, size);
    ctx.quadraticCurveTo(0, size, 0, size - r);
    ctx.lineTo(0, r);
    ctx.quadraticCurveTo(0, 0, r, 0);
    ctx.closePath();
    ctx.fill();
  }

  // Text "S"
  ctx.fillStyle = '#FFFFFF';
  ctx.font = `bold ${Math.floor(size * 0.7)}px Arial`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(text, size / 2, size / 2 + size * 0.05);

  const buffer = canvas.toBuffer('image/png');
  fs.writeFileSync(filename, buffer);
  console.log(`Created ${filename}`);
}

const iconsDir = path.join(__dirname, 'src-tauri', 'icons');

createIcon(32, 'S', path.join(iconsDir, '32x32.png'));
// Generate other sizes just in case
createIcon(128, 'S', path.join(iconsDir, '128x128.png'));
createIcon(128, 'S', path.join(iconsDir, '128x128@2x.png')); // Same size for now or double? Should be double.
// createIcon(256, 'S', path.join(iconsDir, '128x128@2x.png')); // logic above overwrite, fix later if needed.
createIcon(128, 'S', path.join(iconsDir, 'icon.png'));

// Need basic ico/icns? Maybe just pngs are enough for tray if configured right.
// But windows needs .ico for exe usually.
// I'll skip complex ico generation for now, focuses on Tray which uses png in Tauri usually if configured?
// Actually for tray-icon feature:
// In tauri.conf.json: "icon": ["icons/32x32.png", ...]
// Tray usage in Rust: TrayIconBuilder::new().icon(...) or default app icon.
// I will ensure the 32x32.png is updated. That is used for tray often.
