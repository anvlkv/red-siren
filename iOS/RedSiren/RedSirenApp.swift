import SwiftUI
import SharedTypes

extension UIApplication {
    var currentWindow: UIWindow? {
        connectedScenes
            .compactMap {
                $0 as? UIWindowScene
            }
            .flatMap {
                $0.windows
            }
            .first {
                $0.isKeyWindow
            }
    }
}

private struct SafeAreaInsetsKey: EnvironmentKey {
    static var defaultValue: EdgeInsets {
        let val = UIApplication.shared.currentWindow?.safeAreaInsets.swiftUiInsets ?? EdgeInsets()
        return val
    }
}

extension EnvironmentValues {
    var safeAreaInsets: EdgeInsets {
        self[SafeAreaInsetsKey.self]
    }
}

private extension UIEdgeInsets {
    var swiftUiInsets: EdgeInsets {
        EdgeInsets(top: top, leading: left, bottom: bottom, trailing: right)
    }
}

@main
struct iOSApp: App {
    var core: Core
    
    init() {
        self.core = Core()
        self.core.update(Event.start)
    }
    
    var body: some Scene {
        WindowGroup {
            ContentView(core: self.core).environment(\.coreEnv, CoreEnvProvider(core: self.core))
        }
    }
}
