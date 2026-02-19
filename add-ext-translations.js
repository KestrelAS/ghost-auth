const fs = require('fs');
const path = require('path');

const localesDir = path.join(__dirname, 'src', 'lib', 'i18n', 'locales');

const translations = {
  hy: {
    "setup": {
      "heading": "> Ghost Auth",
      "description": "Սահdelays գdelays",
      "passwordPlaceholder": "Գdelays"
    }
  }
};

// Test with Armenian
const hyPath = path.join(localesDir, 'hy.json');
const hyData = JSON.parse(fs.readFileSync(hyPath, 'utf8'));
console.log('Keys:', Object.keys(hyData).length);
console.log('Has ext:', 'ext' in hyData);
