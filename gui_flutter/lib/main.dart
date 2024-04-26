import 'package:flutter/material.dart';
import 'package:gui_flutter/src/rust/api/simple.dart';
import 'package:gui_flutter/src/rust/frb_generated.dart';

Future<void> main() async {
  await RustLib.init();
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: Scaffold(
        appBar: AppBar(title: const Text('flutter_rust_bridge quickstart')),
        body: const Center(
          child: ProfileSelector(),
        ),
      ),
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

class ProfileSelector extends StatefulWidget {
  const ProfileSelector({super.key});

  @override
  State<ProfileSelector> createState() => _ProfileSelectorState();
}

class _ProfileSelectorState extends State<ProfileSelector> {
  List<DropdownMenuEntry<String>> _profiles = [];
  bool _newProfile = false;

  @override
  void initState() {
    super.initState();
    listProfiles().then((value) => _profiles =
        value.map((e) => DropdownMenuEntry(value: e, label: e)).toList());
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Builder(builder: (context) {
        if (!_newProfile) {
          return Center(
            child: Column(
              children: [
                DropdownMenu<String>(
                  dropdownMenuEntries: _profiles,
                ),
                ElevatedButton(
                  child: const Text("Add profile"),
                  onPressed: () => setState(() => _newProfile = true),
                )
              ],
            ),
          );
        } else {
          return Text("Add new profile");
        }
      }),
    );
  }
}
