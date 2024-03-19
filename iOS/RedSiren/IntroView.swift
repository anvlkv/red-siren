import CoreTypes
import SwiftUI
import OSLog
import Foundation


struct IntroViewController: UIViewControllerRepresentable {
    func makeUIViewController(context: Context) -> UIViewController {
        let storyboard = UIStoryboard(name: "Intro", bundle: nil)
        return storyboard.instantiateViewController(withIdentifier: "IntroViewController") // Replace with your view controller identifier
    }

    func updateUIViewController(_ uiViewController: UIViewController, context: Context) {
        
    }
}

struct IntroView: View {
    var opacity: CGFloat

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .topLeading) {
                IntroViewController()
                    .opacity(opacity)
            }
                .frame(width: proxy.frame(in: .global).width, height: proxy.frame(in: .global).height)
        }
            .ignoresSafeArea(.all)
    }
}

