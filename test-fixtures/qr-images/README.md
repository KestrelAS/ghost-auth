Add QR code test images here. Each image should have a matching `.expected.txt` file
containing the exact string the QR code encodes (one line, no trailing newline).

Example:
  google-auth-export.png          : screenshot of a Google Auth export QR code
  google-auth-export.expected.txt : contains "otpauth-migration://offline?data=..."

The test suite in `src/lib/utils/qrImage.test.ts` will automatically pick up
any image + expected-text pair and verify the decoder can extract the content.

Suggested test images:
  - Full-screen iOS screenshot of Google Authenticator export
  - Full-screen Android screenshot of Google Authenticator export
  - Cropped image of just the QR code
  - QR code photographed at an angle
  - Low-contrast or small QR code
  - QR code with a busy/noisy background
