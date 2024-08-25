import 'package:flutter/material.dart';
import 'package:gui_flutter/src/rust/api/client.dart';
import 'package:xterm/xterm.dart';

class TerminalWidget extends StatelessWidget {
  const TerminalWidget({super.key, required this.device});

  final Device device;

  @override
  Widget build(BuildContext context) {
    var terminal = Terminal(
      onResize: (width, height, pixelWidth, pixelHeight) {
        throw UnimplementedError();
      },
      onOutput: (data) {
        throw UnimplementedError();
      },
    );
    return Scaffold(
      appBar: AppBar(
        title: const Text("Terminal"),
      ),
      body: TerminalView(terminal),
    );
  }
}
