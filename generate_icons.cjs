const fs = require('fs');
const { createCanvas } = require('canvas');
const path = require('path');

function createIcon(size, text, filename) {
  const canvas = createCanvas(size, size);
  const ctx = canvas.getContext('2d');

  // Background - Blue gradient effect
  ctx.fillStyle = '#2196F3';

  if (size <= 32) {
    // Simple square for small sizes to ensure visibility
    ctx.fillRect(0, 0, size, size);
  } else {
    // Rounded rect for larger
    const r = size * 0.15;
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

  // Text "S" - white, bold
  ctx.fillStyle = '#FFFFFF';
  ctx.font = `bold ${Math.floor(size * 0.65)}px Arial`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(text, size / 2, size / 2 + size * 0.03);

  const buffer = canvas.toBuffer('image/png');
  fs.writeFileSync(filename, buffer);
  console.log(`Created ${filename} (${size}x${size})`);
}

const iconsDir = path.join(__dirname, 'src-tauri', 'icons');

// Standard Tauri icons
createIcon(32, 'S', path.join(iconsDir, '32x32.png'));
createIcon(128, 'S', path.join(iconsDir, '128x128.png'));
createIcon(256, 'S', path.join(iconsDir, '128x128@2x.png'));
createIcon(128, 'S', path.join(iconsDir, 'icon.png'));

// Windows Store Logos
createIcon(30, 'S', path.join(iconsDir, 'Square30x30Logo.png'));
createIcon(44, 'S', path.join(iconsDir, 'Square44x44Logo.png'));
createIcon(50, 'S', path.join(iconsDir, 'StoreLogo.png'));
createIcon(71, 'S', path.join(iconsDir, 'Square71x71Logo.png'));
createIcon(89, 'S', path.join(iconsDir, 'Square89x89Logo.png'));
createIcon(107, 'S', path.join(iconsDir, 'Square107x107Logo.png'));
createIcon(142, 'S', path.join(iconsDir, 'Square142x142Logo.png'));
createIcon(150, 'S', path.join(iconsDir, 'Square150x150Logo.png'));
createIcon(284, 'S', path.join(iconsDir, 'Square284x284Logo.png'));
createIcon(310, 'S', path.join(iconsDir, 'Square310x310Logo.png'));

console.log('\nAll icons generated successfully!');
console.log('Note: You need to manually create icon.ico and icon.icns for Windows/Mac builds.');
console.log('Use a tool like https://icoconvert.com/ or png2ico to create .ico from 256x256 PNG.');
