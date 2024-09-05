import 'package:flutter/material.dart';
import 'package:gui_flutter/src/rust/api/client.dart';
import 'package:gui_flutter/src/rust/api/client/device.dart';
import 'package:xterm/xterm.dart';

class TerminalWidget extends StatelessWidget {
  const TerminalWidget({super.key, required this.device});

  final Device device;

  Stream<String> _terminalOutputStream(RemoteTerminal remoteTerminal) async* {
    while (true) {
      var next = await remoteTerminal.read();
      if (next == null) {
        return;
      }
      yield next;
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text("Terminal"),
      ),
      body: FutureBuilder(
        future: device.openTerminal(),
        builder: (context, snapshot) {
          if (!snapshot.hasData) {
            return const Center(child: CircularProgressIndicator());
          } else if (snapshot.hasError) {
            return Center(child: Text(snapshot.error.toString()));
          }
          var remoteTerminal = snapshot.data!;

          var terminal = Terminal(
            onResize: (width, height, pixelWidth, pixelHeight) {
              remoteTerminal.resize(
                  size: TerminalSize(cols: width, rows: height));
            },
            onOutput: (data) async {
              print("output: $data");
              await remoteTerminal.write(content: data);
            },
          );

          _terminalOutputStream(remoteTerminal).listen(terminal.write);

          return TerminalView(terminal);
        },
      ),
    );
  }
}
