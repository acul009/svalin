// This file is automatically generated, so please do not edit it.
// Generated by `flutter_rust_bridge`@ 2.3.0.

// ignore_for_file: invalid_use_of_internal_member, unused_import, unnecessary_import

import '../../frb_generated.dart';
import '../../lib.dart';
import '../client.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:freezed_annotation/freezed_annotation.dart' hide protected;
part 'device.freezed.dart';

// These function are ignored because they are on traits that is not defined in current crate (put an empty `#[frb]` on it to unignore): `from`

Stream<RemoteLiveDataRealtimeStatus> deviceSubscribeRealtimeStatus(
        {required Device device}) =>
    RustLib.instance.api
        .crateApiClientDeviceDeviceSubscribeRealtimeStatus(device: device);

// Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<Certificate>>
abstract class Certificate implements RustOpaqueInterface {}

class AgentListItem {
  final PublicAgentData publicData;
  final bool onlineStatus;

  const AgentListItem({
    required this.publicData,
    required this.onlineStatus,
  });

  @override
  int get hashCode => publicData.hashCode ^ onlineStatus.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is AgentListItem &&
          runtimeType == other.runtimeType &&
          publicData == other.publicData &&
          onlineStatus == other.onlineStatus;
}

class CoreStatus {
  final double load;
  final BigInt frequency;

  const CoreStatus({
    required this.load,
    required this.frequency,
  });

  @override
  int get hashCode => load.hashCode ^ frequency.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is CoreStatus &&
          runtimeType == other.runtimeType &&
          load == other.load &&
          frequency == other.frequency;
}

class CpuStatus {
  final List<CoreStatus> cores;

  const CpuStatus({
    required this.cores,
  });

  @override
  int get hashCode => cores.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is CpuStatus &&
          runtimeType == other.runtimeType &&
          cores == other.cores;
}

class PublicAgentData {
  final String name;
  final Certificate cert;

  const PublicAgentData({
    required this.name,
    required this.cert,
  });

  @override
  int get hashCode => name.hashCode ^ cert.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is PublicAgentData &&
          runtimeType == other.runtimeType &&
          name == other.name &&
          cert == other.cert;
}

class RealtimeStatus {
  final CpuStatus cpu;
  final MemoryStatus memory;
  final SwapStatus swap;

  const RealtimeStatus({
    required this.cpu,
    required this.memory,
    required this.swap,
  });

  @override
  int get hashCode => cpu.hashCode ^ memory.hashCode ^ swap.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is RealtimeStatus &&
          runtimeType == other.runtimeType &&
          cpu == other.cpu &&
          memory == other.memory &&
          swap == other.swap;
}

@freezed
sealed class RemoteLiveDataRealtimeStatus with _$RemoteLiveDataRealtimeStatus {
  const RemoteLiveDataRealtimeStatus._();

  const factory RemoteLiveDataRealtimeStatus.unavailable() =
      RemoteLiveDataRealtimeStatus_Unavailable;
  const factory RemoteLiveDataRealtimeStatus.pending() =
      RemoteLiveDataRealtimeStatus_Pending;
  const factory RemoteLiveDataRealtimeStatus.ready(
    RealtimeStatus field0,
  ) = RemoteLiveDataRealtimeStatus_Ready;
}