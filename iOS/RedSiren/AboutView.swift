import SwiftUI
import AVFoundation
import Foundation
import SharedTypes

struct AboutView: View {
    @Environment(\.coreEnv) var core: CoreEnv?

    var vm: IntroVM
    var ev: (IntroEV) -> Void


    let padding = CGFloat(12)
    let gap = CGFloat(12)


    init(vm: IntroVM, ev: @escaping (IntroEV) -> Void) {
        self.vm = vm
        self.ev = ev
    }


    func onLeave() {
        self.core?.update(.menu(.intro))
    }

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .topLeading) {
                IntroViewController()

                RedCardView(position: self.vm.layout.menu_position, flip: 0) {
                    VStack(spacing: self.gap) {
                        Text("About the Red Siren")
                            .font(Font.custom("Rosarivo-Italic", size: 36))
                            .foregroundColor(Color("Main"))
                            .multilineTextAlignment(.center)
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                            .fixedSize(horizontal: false, vertical: true)

                        Text("Red Siren is a noise chime.")
                            .font(Font.custom("Rosarivo-Regular", size: 22))
                            .foregroundColor(Color("Main"))

                        Grid(alignment: .leading, horizontalSpacing: self.gap, verticalSpacing: self.gap) {
                            GridRow {
                                Text("Red")
                                    .font(Font.custom("Rosarivo-Italic", size: 22))
                                    .multilineTextAlignment(.trailing)
                                    .gridColumnAlignment(.trailing)
                                Text("The color red and its many meanings.")
                                    .font(Font.custom("Rosarivo-Regular", size: 22))
                                    .multilineTextAlignment(.leading)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                            GridRow {
                                Text("Siren")
                                    .font(Font.custom("Rosarivo-Italic", size: 22))
                                    .multilineTextAlignment(.trailing)
                                    .gridColumnAlignment(.trailing)
                                Text("Siren - the mythical creature, but also the alarm.")
                                    .font(Font.custom("Rosarivo-Regular", size: 22))
                                    .multilineTextAlignment(.leading)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                            GridRow {
                                Text("is")
                                    .font(Font.custom("Rosarivo-Italic", size: 22))
                                    .multilineTextAlignment(.trailing)
                                    .gridColumnAlignment(.trailing)
                                Text("It exists right now.")
                                    .font(Font.custom("Rosarivo-Regular", size: 22))
                                    .multilineTextAlignment(.leading)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                            GridRow {
                                Text("a")
                                    .font(Font.custom("Rosarivo-Italic", size: 22))
                                    .multilineTextAlignment(.trailing)
                                    .gridColumnAlignment(.trailing)
                                Text("It's a choice, one of many, and therefore any.")
                                    .font(Font.custom("Rosarivo-Regular", size: 22))
                                    .multilineTextAlignment(.leading)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                            GridRow {
                                Text("noise")
                                    .font(Font.custom("Rosarivo-Italic", size: 22))
                                    .multilineTextAlignment(.trailing)
                                    .gridColumnAlignment(.trailing)
                                Text("Random or unwanted sounds.")
                                    .font(Font.custom("Rosarivo-Regular", size: 22))
                                    .multilineTextAlignment(.leading)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                            GridRow {
                                Text("chime")
                                    .font(Font.custom("Rosarivo-Italic", size: 22))
                                    .multilineTextAlignment(.trailing)
                                    .gridColumnAlignment(.trailing)
                                Text("The musical instrument.")
                                    .font(Font.custom("Rosarivo-Regular", size: 22))
                                    .multilineTextAlignment(.leading)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                        }.foregroundColor(Color("Main"))

                        MenuButton(action: onLeave, label: "Clear")
                            .frame(maxWidth: .infinity, maxHeight: .infinity)

                    }
                        .padding(EdgeInsets(
                        top: self.padding, leading: self.padding,
                        bottom: self.padding, trailing: self.padding
                    ))
                }
            }
                .frame(minWidth: proxy.frame(in: .global).width,
                       minHeight: proxy.frame(in: .global).height
            )

        }
            .ignoresSafeArea(.all)
    }
}


