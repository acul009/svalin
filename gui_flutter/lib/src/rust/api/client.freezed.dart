// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'client.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
    'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#adding-getters-and-methods-to-our-models');

/// @nodoc
mixin _$FirstConnect {
  RustOpaqueInterface get field0 => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(Init field0) init,
    required TResult Function(Login field0) login,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(Init field0)? init,
    TResult? Function(Login field0)? login,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(Init field0)? init,
    TResult Function(Login field0)? login,
    required TResult orElse(),
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(FirstConnect_Init value) init,
    required TResult Function(FirstConnect_Login value) login,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(FirstConnect_Init value)? init,
    TResult? Function(FirstConnect_Login value)? login,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(FirstConnect_Init value)? init,
    TResult Function(FirstConnect_Login value)? login,
    required TResult orElse(),
  }) =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $FirstConnectCopyWith<$Res> {
  factory $FirstConnectCopyWith(
          FirstConnect value, $Res Function(FirstConnect) then) =
      _$FirstConnectCopyWithImpl<$Res, FirstConnect>;
}

/// @nodoc
class _$FirstConnectCopyWithImpl<$Res, $Val extends FirstConnect>
    implements $FirstConnectCopyWith<$Res> {
  _$FirstConnectCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;
}

/// @nodoc
abstract class _$$FirstConnect_InitImplCopyWith<$Res> {
  factory _$$FirstConnect_InitImplCopyWith(_$FirstConnect_InitImpl value,
          $Res Function(_$FirstConnect_InitImpl) then) =
      __$$FirstConnect_InitImplCopyWithImpl<$Res>;
  @useResult
  $Res call({Init field0});
}

/// @nodoc
class __$$FirstConnect_InitImplCopyWithImpl<$Res>
    extends _$FirstConnectCopyWithImpl<$Res, _$FirstConnect_InitImpl>
    implements _$$FirstConnect_InitImplCopyWith<$Res> {
  __$$FirstConnect_InitImplCopyWithImpl(_$FirstConnect_InitImpl _value,
      $Res Function(_$FirstConnect_InitImpl) _then)
      : super(_value, _then);

  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? field0 = null,
  }) {
    return _then(_$FirstConnect_InitImpl(
      null == field0
          ? _value.field0
          : field0 // ignore: cast_nullable_to_non_nullable
              as Init,
    ));
  }
}

/// @nodoc

class _$FirstConnect_InitImpl extends FirstConnect_Init {
  const _$FirstConnect_InitImpl(this.field0) : super._();

  @override
  final Init field0;

  @override
  String toString() {
    return 'FirstConnect.init(field0: $field0)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$FirstConnect_InitImpl &&
            (identical(other.field0, field0) || other.field0 == field0));
  }

  @override
  int get hashCode => Object.hash(runtimeType, field0);

  @JsonKey(ignore: true)
  @override
  @pragma('vm:prefer-inline')
  _$$FirstConnect_InitImplCopyWith<_$FirstConnect_InitImpl> get copyWith =>
      __$$FirstConnect_InitImplCopyWithImpl<_$FirstConnect_InitImpl>(
          this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(Init field0) init,
    required TResult Function(Login field0) login,
  }) {
    return init(field0);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(Init field0)? init,
    TResult? Function(Login field0)? login,
  }) {
    return init?.call(field0);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(Init field0)? init,
    TResult Function(Login field0)? login,
    required TResult orElse(),
  }) {
    if (init != null) {
      return init(field0);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(FirstConnect_Init value) init,
    required TResult Function(FirstConnect_Login value) login,
  }) {
    return init(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(FirstConnect_Init value)? init,
    TResult? Function(FirstConnect_Login value)? login,
  }) {
    return init?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(FirstConnect_Init value)? init,
    TResult Function(FirstConnect_Login value)? login,
    required TResult orElse(),
  }) {
    if (init != null) {
      return init(this);
    }
    return orElse();
  }
}

abstract class FirstConnect_Init extends FirstConnect {
  const factory FirstConnect_Init(final Init field0) = _$FirstConnect_InitImpl;
  const FirstConnect_Init._() : super._();

  @override
  Init get field0;
  @JsonKey(ignore: true)
  _$$FirstConnect_InitImplCopyWith<_$FirstConnect_InitImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$FirstConnect_LoginImplCopyWith<$Res> {
  factory _$$FirstConnect_LoginImplCopyWith(_$FirstConnect_LoginImpl value,
          $Res Function(_$FirstConnect_LoginImpl) then) =
      __$$FirstConnect_LoginImplCopyWithImpl<$Res>;
  @useResult
  $Res call({Login field0});
}

/// @nodoc
class __$$FirstConnect_LoginImplCopyWithImpl<$Res>
    extends _$FirstConnectCopyWithImpl<$Res, _$FirstConnect_LoginImpl>
    implements _$$FirstConnect_LoginImplCopyWith<$Res> {
  __$$FirstConnect_LoginImplCopyWithImpl(_$FirstConnect_LoginImpl _value,
      $Res Function(_$FirstConnect_LoginImpl) _then)
      : super(_value, _then);

  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? field0 = null,
  }) {
    return _then(_$FirstConnect_LoginImpl(
      null == field0
          ? _value.field0
          : field0 // ignore: cast_nullable_to_non_nullable
              as Login,
    ));
  }
}

/// @nodoc

class _$FirstConnect_LoginImpl extends FirstConnect_Login {
  const _$FirstConnect_LoginImpl(this.field0) : super._();

  @override
  final Login field0;

  @override
  String toString() {
    return 'FirstConnect.login(field0: $field0)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$FirstConnect_LoginImpl &&
            (identical(other.field0, field0) || other.field0 == field0));
  }

  @override
  int get hashCode => Object.hash(runtimeType, field0);

  @JsonKey(ignore: true)
  @override
  @pragma('vm:prefer-inline')
  _$$FirstConnect_LoginImplCopyWith<_$FirstConnect_LoginImpl> get copyWith =>
      __$$FirstConnect_LoginImplCopyWithImpl<_$FirstConnect_LoginImpl>(
          this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(Init field0) init,
    required TResult Function(Login field0) login,
  }) {
    return login(field0);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(Init field0)? init,
    TResult? Function(Login field0)? login,
  }) {
    return login?.call(field0);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(Init field0)? init,
    TResult Function(Login field0)? login,
    required TResult orElse(),
  }) {
    if (login != null) {
      return login(field0);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(FirstConnect_Init value) init,
    required TResult Function(FirstConnect_Login value) login,
  }) {
    return login(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(FirstConnect_Init value)? init,
    TResult? Function(FirstConnect_Login value)? login,
  }) {
    return login?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(FirstConnect_Init value)? init,
    TResult Function(FirstConnect_Login value)? login,
    required TResult orElse(),
  }) {
    if (login != null) {
      return login(this);
    }
    return orElse();
  }
}

abstract class FirstConnect_Login extends FirstConnect {
  const factory FirstConnect_Login(final Login field0) =
      _$FirstConnect_LoginImpl;
  const FirstConnect_Login._() : super._();

  @override
  Login get field0;
  @JsonKey(ignore: true)
  _$$FirstConnect_LoginImplCopyWith<_$FirstConnect_LoginImpl> get copyWith =>
      throw _privateConstructorUsedError;
}
