// This file is automatically generated, so please do not edit it.
// Generated by `flutter_rust_bridge`@ 2.3.0.

// ignore_for_file: invalid_use_of_internal_member, unused_import, unnecessary_import

import '../frb_generated.dart';
import 'client/device.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:freezed_annotation/freezed_annotation.dart' hide protected;
import 'totp.dart';
part 'client.freezed.dart';

// Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<Client>>
abstract class Client implements RustOpaqueInterface {
  Future<WaitingForConfirmCode> addAgentWithCode({required String joinCode});

  Future<List<Device>> deviceList();

  static Future<FirstConnect> firstConnect({required String address}) =>
      RustLib.instance.api.crateApiClientClientFirstConnect(address: address);

  static Future<List<String>> getProfiles() =>
      RustLib.instance.api.crateApiClientClientGetProfiles();

  static Future<Client> openProfileString(
          {required String profileKey, required String password}) =>
      RustLib.instance.api.crateApiClientClientOpenProfileString(
          profileKey: profileKey, password: password);

  static Future<void> removeProfile({required String profileKey}) =>
      RustLib.instance.api
          .crateApiClientClientRemoveProfile(profileKey: profileKey);
}

// Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<Device>>
abstract class Device implements RustOpaqueInterface {
  Future<AgentListItem> item();

  Future<RemoteTerminal> openTerminal();

  Future<RealtimeStatusReceiver> subscribeRealtime();
}

// Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<Init>>
abstract class Init implements RustOpaqueInterface {
  Future<void> init(
      {required String username,
      required String password,
      required Totp totpSecret});
}

// Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<Login>>
abstract class Login implements RustOpaqueInterface {
  Future<void> login();
}

// Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<TOTP>>
abstract class Totp implements RustOpaqueInterface {
  Future<bool> checkCurrent({required String token});

  Future<Uint8List> getQrPng();

  Future<String> getUrl();
}

// Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<WaitingForConfirmCode>>
abstract class WaitingForConfirmCode implements RustOpaqueInterface {
  Future<void> confirm(
      {required String confirmCode, required String agentName});
}

@freezed
sealed class FirstConnect with _$FirstConnect {
  const FirstConnect._();

  const factory FirstConnect.init(
    Init field0,
  ) = FirstConnect_Init;
  const factory FirstConnect.login(
    Login field0,
  ) = FirstConnect_Login;
}
