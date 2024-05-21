import 'package:flutter/material.dart';
import 'package:gui_flutter/src/rust/frb_generated.dart';
import 'package:gui_flutter/src/app/profile/welcome_screen.dart';

Future<void> main() async {
  await RustLib.init();
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(
      home: ProfileSelector(),
    );
  }
}
