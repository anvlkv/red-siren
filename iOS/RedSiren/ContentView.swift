import CoreTypes
import SwiftUI
import UIScreenExtension
import OSLog

struct SizePreferenceKey: PreferenceKey {
    static var defaultValue: CGSize = .zero
    static func reduce(value: inout CGSize, nextValue: () -> CGSize) {
        value = nextValue()
    }
}


struct ContentView: View {
    @EnvironmentObject var core: Core
    
    @Environment(\.safeAreaInsets) private var safeAreaInsets
    @Environment(\.colorScheme) var colorScheme
    @Environment(\.accessibilityReduceMotion) var reduceMotion

    @StateObject var clock: AnimationClock = AnimationClock()
    
    var body: some View {
        ZStack {
            IntroView(opacity: CGFloat(core.view.visual.intro_opacity))
        }
            .ignoresSafeArea(.all)
            .statusBarHidden(true)
            .overlay(
            GeometryReader { proxy in
                Color.clear.preference(key: SizePreferenceKey.self, value: proxy.frame(in: .global).size)
            }
                .ignoresSafeArea(.all)
        )
            .onAppear {
            Logger().log("set cbs")
            core.startClock = { cb in
                self.clock.onTick = cb
                self.clock.createDisplayLink()
                Logger().log("starting")
            }
            core.stopClock = {
                self.clock.deleteDisplayLink()
            }

            core.update(.visual(.setDarkMode(colorScheme == .dark)))
            core.update(.visual(.setDensity(UIScreen.pixelsPerInch ?? 1.0)))
            core.update(.visual(.safeAreaResize(
                safeAreaInsets.leading,
                safeAreaInsets.top,
                safeAreaInsets.trailing,
                safeAreaInsets.bottom
            )))
            core.update(.visual(.setReducedMotion(reduceMotion)))
        }
            .onDisappear {
            core.startClock = nil
            core.stopClock = nil
        }
            .onChange(of: colorScheme, perform: { scheme in
            core.update(.visual(.setDarkMode(scheme == .dark)))
        })
            .onChange(of: UIScreen.pixelsPerInch ?? 1.0, perform: { ppi in
            core.update(.visual(.setDensity(ppi)))
        })
            .onChange(of: safeAreaInsets, perform: { insets in
            core.update(.visual(.safeAreaResize(
                insets.leading,
                insets.top,
                insets.trailing,
                insets.bottom
            )))
        })
            .onChange(of: reduceMotion, perform: { reduce in
            core.update(.visual(.setReducedMotion(reduce)))
        })
            .onPreferenceChange(SizePreferenceKey.self) { size in
            core.update(.visual(.resize(size.width, size.height)))
        }
            .background(Color("Main"))
    }
}
