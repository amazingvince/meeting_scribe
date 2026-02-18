import Foundation
import AudioToolbox
import AVFoundation

private let targetSampleRate: Double = 16_000

private struct CaptureError: Error, CustomStringConvertible {
    let description: String
}

private extension AudioObjectID {
    static let system = AudioObjectID(kAudioObjectSystemObject)
    static let unknown = kAudioObjectUnknown

    var isValid: Bool { self != .unknown }

    static func readDefaultOutputDevice() throws -> AudioDeviceID {
        try AudioObjectID.system.read(
            kAudioHardwarePropertyDefaultOutputDevice,
            defaultValue: AudioDeviceID.unknown
        )
    }

    func readDeviceUID() throws -> String {
        try readString(kAudioDevicePropertyDeviceUID)
    }

    func readAudioTapStreamBasicDescription() throws -> AudioStreamBasicDescription {
        try read(kAudioTapPropertyFormat, defaultValue: AudioStreamBasicDescription())
    }

    func read<T>(
        _ selector: AudioObjectPropertySelector,
        scope: AudioObjectPropertyScope = kAudioObjectPropertyScopeGlobal,
        element: AudioObjectPropertyElement = kAudioObjectPropertyElementMain,
        defaultValue: T
    ) throws -> T {
        var address = AudioObjectPropertyAddress(
            mSelector: selector,
            mScope: scope,
            mElement: element
        )

        var dataSize: UInt32 = 0
        var status = AudioObjectGetPropertyDataSize(self, &address, 0, nil, &dataSize)
        guard status == noErr else {
            throw CaptureError(description: "AudioObjectGetPropertyDataSize(\(selector)) failed: \(status)")
        }

        var value = defaultValue
        status = withUnsafeMutablePointer(to: &value) { ptr in
            AudioObjectGetPropertyData(self, &address, 0, nil, &dataSize, ptr)
        }
        guard status == noErr else {
            throw CaptureError(description: "AudioObjectGetPropertyData(\(selector)) failed: \(status)")
        }

        return value
    }

    func readString(
        _ selector: AudioObjectPropertySelector,
        scope: AudioObjectPropertyScope = kAudioObjectPropertyScopeGlobal,
        element: AudioObjectPropertyElement = kAudioObjectPropertyElementMain
    ) throws -> String {
        let value: CFString = try read(selector, scope: scope, element: element, defaultValue: "" as CFString)
        return value as String
    }
}

private final class ProcessTapCapture {
    private let outputHandle = FileHandle.standardOutput
    private let callbackQueue = DispatchQueue(label: "meeting-scribe.process-tap-callback", qos: .userInitiated)

    private var tapID: AudioObjectID = .unknown
    private var aggregateDeviceID: AudioObjectID = .unknown
    private var deviceProcID: AudioDeviceIOProcID?

    private var inputFormat: AVAudioFormat?
    private var outputFormat: AVAudioFormat
    private var converter: AVAudioConverter?
    private var didLogSourceSelection = false

    init() throws {
        guard let format = AVAudioFormat(
            commonFormat: .pcmFormatFloat32,
            sampleRate: targetSampleRate,
            channels: 1,
            interleaved: false
        ) else {
            throw CaptureError(description: "Failed to initialize output format")
        }
        self.outputFormat = format
    }

    func start() throws {
        try ensureSupportedOS()
        try ensureAudioCapturePermission()

        let tapDescription = CATapDescription(stereoGlobalTapButExcludeProcesses: [])
        tapDescription.uuid = UUID()
        tapDescription.muteBehavior = .unmuted

        var createdTapID: AudioObjectID = .unknown
        var status = AudioHardwareCreateProcessTap(tapDescription, &createdTapID)
        guard status == noErr else {
            throw CaptureError(description: "AudioHardwareCreateProcessTap failed: \(status)")
        }
        tapID = createdTapID

        let outputDeviceID = try AudioObjectID.readDefaultOutputDevice()
        guard outputDeviceID != .unknown else {
            throw CaptureError(description: "Default output device not found")
        }
        let outputUID = try outputDeviceID.readDeviceUID()

        let aggregateUID = UUID().uuidString
        let aggregateDescription: [String: Any] = [
            kAudioAggregateDeviceNameKey: "MeetingScribe-SystemTap",
            kAudioAggregateDeviceUIDKey: aggregateUID,
            kAudioAggregateDeviceMainSubDeviceKey: outputUID,
            kAudioAggregateDeviceIsPrivateKey: true,
            kAudioAggregateDeviceIsStackedKey: false,
            kAudioAggregateDeviceTapAutoStartKey: true,
            kAudioAggregateDeviceSubDeviceListKey: [
                [
                    kAudioSubDeviceUIDKey: outputUID
                ]
            ],
            kAudioAggregateDeviceTapListKey: [
                [
                    kAudioSubTapDriftCompensationKey: true,
                    kAudioSubTapUIDKey: tapDescription.uuid.uuidString
                ]
            ]
        ]

        var tapDescriptionFormat = try tapID.readAudioTapStreamBasicDescription()
        guard let avInputFormat = AVAudioFormat(streamDescription: &tapDescriptionFormat) else {
            throw CaptureError(description: "Failed to initialize AVAudioFormat from tap stream description")
        }
        inputFormat = avInputFormat

        guard let avConverter = AVAudioConverter(from: avInputFormat, to: outputFormat) else {
            throw CaptureError(description: "Failed to create AVAudioConverter for tap stream")
        }
        converter = avConverter

        status = AudioHardwareCreateAggregateDevice(aggregateDescription as CFDictionary, &aggregateDeviceID)
        guard status == noErr else {
            throw CaptureError(description: "AudioHardwareCreateAggregateDevice failed: \(status)")
        }

        status = AudioDeviceCreateIOProcIDWithBlock(
            &deviceProcID,
            aggregateDeviceID,
            callbackQueue
        ) { [weak self] _, inInputData, _, outOutputData, _ in
            self?.handleInput(inInputData, outputData: outOutputData)
        }
        guard status == noErr else {
            throw CaptureError(description: "AudioDeviceCreateIOProcIDWithBlock failed: \(status)")
        }

        status = AudioDeviceStart(aggregateDeviceID, deviceProcID)
        guard status == noErr else {
            throw CaptureError(description: "AudioDeviceStart failed: \(status)")
        }
    }

    func stop() {
        if aggregateDeviceID.isValid {
            _ = AudioDeviceStop(aggregateDeviceID, deviceProcID)
        }

        if let procID = deviceProcID, aggregateDeviceID.isValid {
            _ = AudioDeviceDestroyIOProcID(aggregateDeviceID, procID)
            deviceProcID = nil
        }

        if aggregateDeviceID.isValid {
            _ = AudioHardwareDestroyAggregateDevice(aggregateDeviceID)
            aggregateDeviceID = .unknown
        }

        if tapID.isValid {
            _ = AudioHardwareDestroyProcessTap(tapID)
            tapID = .unknown
        }
    }

    deinit {
        stop()
    }

    private func handleInput(
        _ inputData: UnsafePointer<AudioBufferList>?,
        outputData: UnsafeMutablePointer<AudioBufferList>?
    ) {
        guard
            let inputFormat,
            let converter
        else {
            return
        }

        let outputDataConst = outputData.map { UnsafePointer<AudioBufferList>($0) }
        let inputHasSignal = inputData.map(bufferListHasSignal) ?? false
        let outputHasSignal = outputDataConst.map(bufferListHasSignal) ?? false

        let selectedList: UnsafePointer<AudioBufferList>?
        let selectedLabel: String
        if inputHasSignal || !outputHasSignal {
            selectedList = inputData
            selectedLabel = "input"
        } else {
            selectedList = outputDataConst
            selectedLabel = "output"
        }

        if !didLogSourceSelection {
            didLogSourceSelection = true
            writeStderr(
                "SOURCE: selected=\(selectedLabel) input_signal=\(inputHasSignal) output_signal=\(outputHasSignal)"
            )
        }

        guard let selectedList else {
            return
        }

        guard let inputBuffer = AVAudioPCMBuffer(
            pcmFormat: inputFormat,
            bufferListNoCopy: selectedList,
            deallocator: nil
        ) else {
            return
        }

        let ratio = outputFormat.sampleRate / inputFormat.sampleRate
        let outFrameCapacity = AVAudioFrameCount(
            max(32, Int(ceil(Double(inputBuffer.frameLength) * ratio)) + 32)
        )

        guard let outputBuffer = AVAudioPCMBuffer(
            pcmFormat: outputFormat,
            frameCapacity: outFrameCapacity
        ) else {
            return
        }

        var conversionError: NSError?
        var providedInput = false

        let status = converter.convert(to: outputBuffer, error: &conversionError) { _, outStatus in
            if providedInput {
                outStatus.pointee = .noDataNow
                return nil
            }
            providedInput = true
            outStatus.pointee = .haveData
            return inputBuffer
        }

        if status == .error {
            if let conversionError {
                writeStderr("ERROR: Converter failed: \(conversionError.localizedDescription)")
            }
            return
        }

        let frameLength = Int(outputBuffer.frameLength)
        guard frameLength > 0, let channelData = outputBuffer.floatChannelData?[0] else {
            return
        }

        let byteCount = frameLength * MemoryLayout<Float>.size
        let payload = Data(bytes: channelData, count: byteCount)

        do {
            try outputHandle.write(contentsOf: payload)
        } catch {
            // Parent likely closed the pipe during shutdown.
        }
    }

    private func bufferListHasSignal(_ list: UnsafePointer<AudioBufferList>) -> Bool {
        let mutable = UnsafeMutablePointer<AudioBufferList>(mutating: list)
        let buffers = UnsafeMutableAudioBufferListPointer(mutable)
        for buffer in buffers {
            guard let data = buffer.mData, buffer.mDataByteSize > 0 else {
                continue
            }
            let count = Int(min(buffer.mDataByteSize, 4096))
            let bytes = data.assumingMemoryBound(to: UInt8.self)
            for index in 0..<count {
                if bytes[index] != 0 {
                    return true
                }
            }
        }
        return false
    }
}

private func ensureSupportedOS() throws {
    let version = ProcessInfo.processInfo.operatingSystemVersion
    let supported = version.majorVersion > 14 || (version.majorVersion == 14 && version.minorVersion >= 2)
    guard supported else {
        throw CaptureError(
            description: "CoreAudio Process Tap requires macOS 14.2+, found \(version.majorVersion).\(version.minorVersion).\(version.patchVersion)"
        )
    }
}

private func ensureAudioCapturePermission() throws {
    typealias PreflightFn = @convention(c) (CFString, CFDictionary?) -> Int
    typealias RequestFn = @convention(c) (CFString, CFDictionary?, @escaping (Bool) -> Void) -> Void

    let path = "/System/Library/PrivateFrameworks/TCC.framework/Versions/A/TCC"
    guard let handle = dlopen(path, RTLD_NOW) else {
        // If TCC SPI is unavailable, continue and let CoreAudio report any failure.
        return
    }

    guard
        let preflightSym = dlsym(handle, "TCCAccessPreflight"),
        let requestSym = dlsym(handle, "TCCAccessRequest")
    else {
        return
    }

    let preflight = unsafeBitCast(preflightSym, to: PreflightFn.self)
    let request = unsafeBitCast(requestSym, to: RequestFn.self)

    let service = "kTCCServiceAudioCapture" as CFString
    let status = preflight(service, nil)
    if status == 0 {
        return
    }

    let semaphore = DispatchSemaphore(value: 0)
    var granted = false
    request(service, nil) { result in
        granted = result
        semaphore.signal()
    }

    _ = semaphore.wait(timeout: .now() + 5)

    guard granted else {
        throw CaptureError(
            description: """
            System audio permission denied (kTCCServiceAudioCapture).
            If running in dev from a terminal app, macOS may attribute permission to the terminal bundle.
            Run the built Meeting Scribe .app and grant System Audio Recording permission in System Settings.
            """
        )
    }
}

private func writeStderr(_ message: String) {
    if let data = "\(message)\n".data(using: .utf8) {
        FileHandle.standardError.write(data)
    }
}

@main
struct MeetingScribeProcessTapHelper {
    static func main() {
        signal(SIGPIPE, SIG_IGN)
        signal(SIGINT, SIG_IGN)
        signal(SIGTERM, SIG_IGN)

        do {
            let capture = try ProcessTapCapture()
            try capture.start()
            writeStderr("READY: Process Tap streaming at \(Int(targetSampleRate))Hz mono float32")

            let semaphore = DispatchSemaphore(value: 0)
            let stopQueue = DispatchQueue(label: "meeting-scribe.process-tap-stop")
            var hasStopped = false

            let stopHandler = {
                stopQueue.sync {
                    if hasStopped {
                        return
                    }
                    hasStopped = true
                    capture.stop()
                    semaphore.signal()
                }
            }

            let signalQueue = DispatchQueue(label: "meeting-scribe.process-tap-signals")
            let sigInt = DispatchSource.makeSignalSource(signal: SIGINT, queue: signalQueue)
            sigInt.setEventHandler(handler: stopHandler)
            sigInt.resume()

            let sigTerm = DispatchSource.makeSignalSource(signal: SIGTERM, queue: signalQueue)
            sigTerm.setEventHandler(handler: stopHandler)
            sigTerm.resume()

            semaphore.wait()
            sigInt.cancel()
            sigTerm.cancel()
        } catch {
            writeStderr("ERROR: \(error)")
            exit(1)
        }
    }
}
