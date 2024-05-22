// This file is automatically generated, so please do not edit it.
// Generated by `flutter_rust_bridge`@ 2.0.0-dev.35.

// ignore_for_file: invalid_use_of_internal_member, unused_import, unnecessary_import

import '../frb_generated.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:freezed_annotation/freezed_annotation.dart' hide protected;
part 'client.freezed.dart';

// The type `__external_impl__436c69656e74` is not used by any `pub` functions, thus it is ignored.
// The type `__external_impl__496e6974` is not used by any `pub` functions, thus it is ignored.
// The type `__external_impl__4c6f67696e` is not used by any `pub` functions, thus it is ignored.

Future<String> sayHello({dynamic hint}) =>
    RustLib.instance.api.crateApiClientSayHello(hint: hint);

// Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<Client>>
@sealed
class Client extends RustOpaque {
  Client.dcoDecode(List<dynamic> wire) : super.dcoDecode(wire, _kStaticData);

  Client.sseDecode(int ptr, int externalSizeOnNative)
      : super.sseDecode(ptr, externalSizeOnNative, _kStaticData);

  static final _kStaticData = RustArcStaticData(
    rustArcIncrementStrongCount:
        RustLib.instance.api.rust_arc_increment_strong_count_Client,
    rustArcDecrementStrongCount:
        RustLib.instance.api.rust_arc_decrement_strong_count_Client,
    rustArcDecrementStrongCountPtr:
        RustLib.instance.api.rust_arc_decrement_strong_count_ClientPtr,
  );

  static Future<FirstConnect> firstConnect(
          {required String address, dynamic hint}) =>
      RustLib.instance.api
          .crateApiClientClientFirstConnect(address: address, hint: hint);

  static Future<List<String>> getProfiles({dynamic hint}) =>
      RustLib.instance.api.crateApiClientClientGetProfiles(hint: hint);

  static Future<Client> openProfileString(
          {required String profileKey,
          required String password,
          dynamic hint}) =>
      RustLib.instance.api.crateApiClientClientOpenProfileString(
          profileKey: profileKey, password: password, hint: hint);

  static Future<void> removeProfile(
          {required String profileKey, dynamic hint}) =>
      RustLib.instance.api.crateApiClientClientRemoveProfile(
          profileKey: profileKey, hint: hint);
}

// Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<Init>>
@sealed
class Init extends RustOpaque {
  Init.dcoDecode(List<dynamic> wire) : super.dcoDecode(wire, _kStaticData);

  Init.sseDecode(int ptr, int externalSizeOnNative)
      : super.sseDecode(ptr, externalSizeOnNative, _kStaticData);

  static final _kStaticData = RustArcStaticData(
    rustArcIncrementStrongCount:
        RustLib.instance.api.rust_arc_increment_strong_count_Init,
    rustArcDecrementStrongCount:
        RustLib.instance.api.rust_arc_decrement_strong_count_Init,
    rustArcDecrementStrongCountPtr:
        RustLib.instance.api.rust_arc_decrement_strong_count_InitPtr,
  );

  Future<void> init(
          {required String username,
          required String password,
          required Totp totpSecret,
          dynamic hint}) =>
      RustLib.instance.api.crateApiClientInitInit(
          that: this,
          username: username,
          password: password,
          totpSecret: totpSecret,
          hint: hint);
}

// Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<Login>>
@sealed
class Login extends RustOpaque {
  Login.dcoDecode(List<dynamic> wire) : super.dcoDecode(wire, _kStaticData);

  Login.sseDecode(int ptr, int externalSizeOnNative)
      : super.sseDecode(ptr, externalSizeOnNative, _kStaticData);

  static final _kStaticData = RustArcStaticData(
    rustArcIncrementStrongCount:
        RustLib.instance.api.rust_arc_increment_strong_count_Login,
    rustArcDecrementStrongCount:
        RustLib.instance.api.rust_arc_decrement_strong_count_Login,
    rustArcDecrementStrongCountPtr:
        RustLib.instance.api.rust_arc_decrement_strong_count_LoginPtr,
  );

  Future<void> login({dynamic hint}) =>
      RustLib.instance.api.crateApiClientLoginLogin(that: this, hint: hint);
}

// Rust type: RustOpaqueMoi<flutter_rust_bridge::for_generated::RustAutoOpaqueInner<TOTP>>
@sealed
class Totp extends RustOpaque {
  Totp.dcoDecode(List<dynamic> wire) : super.dcoDecode(wire, _kStaticData);

  Totp.sseDecode(int ptr, int externalSizeOnNative)
      : super.sseDecode(ptr, externalSizeOnNative, _kStaticData);

  static final _kStaticData = RustArcStaticData(
    rustArcIncrementStrongCount:
        RustLib.instance.api.rust_arc_increment_strong_count_Totp,
    rustArcDecrementStrongCount:
        RustLib.instance.api.rust_arc_decrement_strong_count_Totp,
    rustArcDecrementStrongCountPtr:
        RustLib.instance.api.rust_arc_decrement_strong_count_TotpPtr,
  );

  Future<bool> checkCurrent({required String token, dynamic hint}) =>
      RustLib.instance.api
          .crateApiTotpTotpCheckCurrent(that: this, token: token, hint: hint);

  Future<Uint8List> getQrPng({dynamic hint}) =>
      RustLib.instance.api.crateApiTotpTotpGetQrPng(that: this, hint: hint);

  Future<String> getUrl({dynamic hint}) =>
      RustLib.instance.api.crateApiTotpTotpGetUrl(that: this, hint: hint);
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