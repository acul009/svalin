import 'package:flutter/material.dart';
import 'package:gui_flutter/src/rust/api/simple.dart';
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
    return MaterialApp(
      home: ProfileSelector(),
    );
  }
}

class Timer extends StatefulWidget {
  const Timer({super.key});

  @override
  State<Timer> createState() => _TimerState();
}

class _TimerState extends State<Timer> {
  late Stream<String> timeStream;

  @override
  void initState() {
    super.initState();
    timeStream = streamTime();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(
          child: Column(
        children: <Widget>[
          const Text("Current Time:"),
          StreamBuilder<String>(
              stream: timeStream,
              builder: (context, snap) {
                final style = Theme.of(context).textTheme.headlineMedium;
                final data = snap.data;
                if (data != null) return Text(data, style: style);

                return const Text("data is null");
              })
        ],
      )),
    );
  }
}
