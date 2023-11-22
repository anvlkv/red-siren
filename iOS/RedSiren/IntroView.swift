import SharedTypes
import SwiftUI

import Foundation


struct IntroViewController: UIViewControllerRepresentable {
    func makeUIViewController(context: Context) -> UIViewController {
        let storyboard = UIStoryboard(name: "Intro", bundle: nil)
        return storyboard.instantiateViewController(withIdentifier: "IntroViewController") // Replace with your view controller identifier
    }

    func updateUIViewController(_ uiViewController: UIViewController, context: Context) {
        // Update the view controller if needed
    }
}

struct IntroView: View {
    var vm: IntroVM
    var ev: (IntroEV) -> Void

    @StateObject var clock: AnimationClock = AnimationClock()
    private var observer: NSKeyValueObservation!


    @Environment(\.accessibilityReduceMotion) var reduceMotion

    var hSize: CGFloat
    var vSize: CGFloat



    init(vm: IntroVM, ev: @escaping (IntroEV) -> Void) {
        self.vm = vm
        self.ev = ev


        self.hSize = CGFloat(vm.view_box.rect[1][0])
        self.vSize = CGFloat(vm.view_box.rect[1][1])
    }



    var body: some View {
        GeometryReader {proxy in
            ZStack(alignment: .topLeading) {
                FluteGroup(inbound: self.vm.layout.inbound,
                           outbound: self.vm.layout.outbound,
                           rotation: self.vm.flute_rotation,
                           offset: self.vm.flute_position,
                           vSize: self.vSize,
                           hSize: self.hSize
                )
                    .opacity(Double(1 - self.vm.intro_opacity))
                
                TracksGroup(tracks: self.vm.layout.tracks, vSize: self.vSize, hSize: self.hSize)
                    .opacity(Double(1 - self.vm.intro_opacity))
                
                ButtonsGroup(buttons: self.vm.layout.buttons, offset: self.vm.buttons_position, vSize: self.vSize, hSize: self.hSize)
                    .opacity(Double(1 - self.vm.intro_opacity))
                    
                
                IntroViewController()
                    .opacity(Double(self.vm.intro_opacity))
            }
                .onAppear {
                    self.clock.onTick = { ts in
                        ev(IntroEV.tsNext(ts))
                    }
                    self.clock.onStart = { ts in
                        ev(IntroEV.startAnimation(ts_start: ts, reduced_motion: self.reduceMotion))
                    }

                    self.clock.createDisplayLink()
                }
                .onDisappear {
                    self.clock.deleteDisplayLink()
                }
                .frame(width: proxy.frame(in: .global).width, height: proxy.frame(in: .global).height)
        }
        .ignoresSafeArea(.all)
    }
}



struct FluteGroup: View {

    var inbound: Line
    var outbound: Line
    var fluteRotationAnchor: UnitPoint
    var fluteOffset: CGSize
    var fluteRotation: Angle
    var vSize: CGFloat
    var hSize: CGFloat


    init(inbound: Line, outbound: Line, rotation: [Double], offset: [Double], vSize: CGFloat, hSize: CGFloat) {
        self.inbound = inbound
        self.outbound = outbound
        self.vSize = vSize
        self.hSize = hSize
        self.fluteRotation = Angle(degrees: rotation[2])
        self.fluteRotationAnchor = UnitPoint(x: CGFloat(rotation[0] / self.hSize), y: CGFloat(rotation[1] / self.vSize))
        self.fluteOffset = CGSize(width: offset[0], height: offset[1])

    }

    var body: some View {
        GeometryReader{ctx in
            ZStack(alignment: .bottomTrailing) {
                Group {
                    Group {
                        InstrumentInboundStringView(line: self.inbound)
                        InstrumentOutboundStringView(line: self.outbound)
                    }
                        .offset(self.fluteOffset)
                        .rotationEffect(self.fluteRotation, anchor: self.fluteRotationAnchor)
                }
                    .frame(width: hSize, height: vSize)
                
                Rectangle()
                    .size(width: ctx.size.width, height: ctx.size.height)
                    .opacity(0.0)
                    
            }.frame(width: ctx.size.width, height: ctx.size.height)
        }
        
    }

}

struct TracksGroup: View {
    var vSize: CGFloat
    var hSize: CGFloat
    var tracks: [Rect]

    init(tracks: [Rect], vSize: CGFloat, hSize: CGFloat) {
        self.hSize = hSize
        self.vSize = vSize
        self.tracks = tracks
    }

    var body: some View {
        ZStack {
            ForEach(self.tracks, id: \.hashValue) { track in
                InstrumentTrackView(rect: track)
            }
        }
        .frame(width: self.hSize, height: self.vSize, alignment: .topLeading)
    }
}

struct ButtonsGroup: View {
    var buttons: [Rect]
    var vSize: CGFloat
    var hSize: CGFloat
    var offset: CGSize

    init(buttons: [Rect], offset: [Double], vSize: CGFloat, hSize: CGFloat) {
        self.buttons = buttons
        self.offset = CGSize(width: offset[0], height: offset[1])
        self.vSize = vSize
        self.hSize = hSize
    }

    var body: some View {
        ZStack {
            ForEach(self.buttons, id: \.hashValue) { btn in
                InstrumentButtonView(rect: btn)
            }
        }
            .frame(width: self.hSize, height: self.vSize, alignment: .topLeading)
            .offset(self.offset)
    }
}
