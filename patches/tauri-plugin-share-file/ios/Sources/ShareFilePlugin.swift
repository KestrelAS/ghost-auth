import UIKit
import Tauri

struct ShareFileArgs: Decodable {
    let path: String
    let mimeType: String
}

class ShareFilePlugin: Plugin {

    @objc func shareFile(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(ShareFileArgs.self)
        let fileURL = URL(fileURLWithPath: args.path)

        guard FileManager.default.fileExists(atPath: args.path) else {
            invoke.reject("File not found at path: \(args.path)")
            return
        }

        DispatchQueue.main.async {
            guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
                  let rootVC = scene.windows.first?.rootViewController else {
                invoke.reject("Could not find root view controller")
                return
            }

            let activityVC = UIActivityViewController(
                activityItems: [fileURL],
                applicationActivities: nil
            )

            // iPad requires popover configuration or it will crash
            if let popover = activityVC.popoverPresentationController {
                popover.sourceView = rootVC.view
                popover.sourceRect = CGRect(
                    x: rootVC.view.bounds.midX,
                    y: rootVC.view.bounds.midY,
                    width: 0, height: 0
                )
                popover.permittedArrowDirections = []
            }

            activityVC.completionWithItemsHandler = { _, completed, _, error in
                // Clean up the temp file regardless of outcome
                try? FileManager.default.removeItem(at: fileURL)

                if let error = error {
                    invoke.reject("Share failed: \(error.localizedDescription)")
                } else {
                    invoke.resolve(["shared": completed])
                }
            }

            rootVC.present(activityVC, animated: true)
        }
    }
}

@_cdecl("init_plugin_share_file")
func initPlugin() -> Plugin {
    return ShareFilePlugin()
}
