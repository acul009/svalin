import 'package:flutter/material.dart';
import 'package:gui_flutter/src/rust/api/simple.dart';

class ServerDialog extends StatefulWidget {
  const ServerDialog({super.key});

  @override
  State<ServerDialog> createState() => _ServerDialogState();
}

class _ServerDialogState extends State<ServerDialog> {
  String _serverAddress = "";

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text("New Profile"),
      ),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 50, vertical: 20),
          child: Column(children: [
            TextFormField(
              decoration: const InputDecoration(
                border: OutlineInputBorder(),
                labelText: "Server Address",
              ),
              onChanged: (value) => setState(() => _serverAddress = value),
            ),
            const SizedBox(height: 20),
            ElevatedButton(
              style: ElevatedButton.styleFrom(
                minimumSize: const Size.fromHeight(50),
              ),
              child: const Text("Connect"),
              onPressed: () {
                Navigator.push(context, MaterialPageRoute(builder: (context) {
                  return ConnectingDialog(address: _serverAddress);
                }));
              },
            ),
          ]),
        ),
      ),
    );
  }
}

class ConnectingDialog extends StatelessWidget {
  const ConnectingDialog({super.key, required this.address});

  final String address;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(
        child: FutureBuilder(
          future: firstConnect(address: address),
          builder: (context, snapshot) {
            if (snapshot.hasData) {
              return Text("Connected");
            } else if (snapshot.error != null) {
              return Text(snapshot.error.toString());
            } else {
              return const CircularProgressIndicator();
            }
          },
        ),
      ),
    );
  }
}
