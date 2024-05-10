import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:gui_flutter/src/rust/api/client.dart';
import 'package:gui_flutter/src/rust/api/simple.dart';
import 'package:gui_flutter/src/rust/api/totp.dart';

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
              child: const Text("Connect To Server"),
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
    var connecting = Client.firstConnect(address: address);
    connecting.then((value) {
      switch (value) {
        case FirstConnect_Init(field0: final init):
          Navigator.pushReplacement(
            context,
            MaterialPageRoute(
                builder: (context) => RegisterRootDialog(connection: init)),
          );
        case FirstConnect_Login():
        // TODO: Handle this case.
      }
    });

    return Scaffold(
      appBar: AppBar(),
      body: const Center(child: CircularProgressIndicator()),
    );
  }
}

class RegisterRootDialog extends StatefulWidget {
  const RegisterRootDialog({super.key, required this.connection});

  final Init connection;

  @override
  State<StatefulWidget> createState() {
    return _RegisterRootDialogState();
  }
}

class _RegisterRootDialogState extends State<RegisterRootDialog> {
  String _username = "";
  String _password = "";

  final _formKey = GlobalKey<FormState>();

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text("Register root-user"),
      ),
      body: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 50, vertical: 20),
        child: Form(
          key: _formKey,
          child: Column(
            children: [
              TextFormField(
                decoration: const InputDecoration(
                  border: OutlineInputBorder(),
                  labelText: "username",
                ),
                onChanged: (value) => setState(() {
                  _username = value;
                }),
                validator: (value) =>
                    value == null || value.isEmpty ? "required" : null,
              ),
              const SizedBox(height: 20),
              TextFormField(
                obscureText: true,
                decoration: const InputDecoration(
                  border: OutlineInputBorder(),
                  labelText: "password",
                ),
                onChanged: (value) => setState(() {
                  _password = value;
                }),
              ),
              const SizedBox(height: 20),
              TextFormField(
                obscureText: true,
                decoration: const InputDecoration(
                  border: OutlineInputBorder(),
                  labelText: "repeat password",
                ),
                validator: (value) =>
                    value == _password ? null : "Passwords must match!",
              ),
              const SizedBox(height: 20),
              ElevatedButton(
                style: ElevatedButton.styleFrom(
                  minimumSize: const Size.fromHeight(50),
                ),
                onPressed: () {
                  if (_formKey.currentState!.validate()) {
                    Navigator.push(
                        context,
                        MaterialPageRoute(
                          builder: (context) => CreateTotpDialog(
                              connection: widget.connection,
                              username: _username,
                              password: _password),
                        ));
                  }
                },
                child: const Text("Next"),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class CreateTotpDialog extends StatefulWidget {
  const CreateTotpDialog(
      {super.key,
      required this.connection,
      required this.username,
      required this.password});

  final Init connection;
  final String username;
  final String password;

  @override
  State<StatefulWidget> createState() {
    return _CreateTotpDialogState();
  }
}

class _CreateTotpDialogState extends State<CreateTotpDialog> {
  late Future<Totp> totpFuture;

  final _formKey = GlobalKey<FormState>();

  String totpToken = "";

  @override
  void initState() {
    super.initState();

    totpFuture = newTotp(accountName: widget.username);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text("Setup TOTP"),
      ),
      body: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 50, vertical: 20),
        child: FutureBuilder(
          future: totpFuture,
          builder: (context, snapshot) {
            if (snapshot.hasData) {
              var totp = snapshot.data!;
              return Form(
                key: _formKey,
                child: Center(
                  child: Column(
                    children: [
                      FutureBuilder(
                        future: totp.getQrPng(),
                        builder: (context, snapshot) {
                          if (snapshot.hasData) {
                            return Image.memory(snapshot.data!);
                          } else {
                            return const Center(
                                child: CircularProgressIndicator());
                          }
                        },
                      ),
                      const SizedBox(height: 20),
                      ElevatedButton(
                        style: ElevatedButton.styleFrom(
                          minimumSize: const Size.fromHeight(50),
                        ),
                        onPressed: () async {
                          var url = await totp.getUrl();
                          await Clipboard.setData(ClipboardData(text: url));
                        },
                        child: const Text("Copy TOTP Secret instead"),
                      ),
                      const Divider(height: 100),
                      TextFormField(
                          obscureText: true,
                          decoration: const InputDecoration(
                            border: OutlineInputBorder(),
                            labelText: "input current totp token",
                          ),
                          onChanged: (value) => setState(() {
                                totpToken = value;
                              }),
                          validator: (value) {
                            RegExp regex = RegExp(r'[0-9]{8}');
                            if (value == null || value.isEmpty) {
                              return "required";
                            } else if (!RegExp(r'^[0-9]{8}$').hasMatch(value)) {
                              return "not a TOTP token";
                            } else {
                              return null;
                            }
                          }),
                      const SizedBox(height: 20),
                      ElevatedButton(
                        style: ElevatedButton.styleFrom(
                          minimumSize: const Size.fromHeight(50),
                        ),
                        onPressed: () async {
                          if (_formKey.currentState!.validate()) {
                            if (!await totp.checkCurrent(token: totpToken)) {
                              if (context.mounted) {
                                ScaffoldMessenger.of(context)
                                    .showSnackBar(const SnackBar(
                                  backgroundColor: Colors.red,
                                  content: Text(
                                      "TOTP-token not valid.\nPlease try again!"),
                                ));
                              }
                            } else {
                              if (context.mounted) {
                                Navigator.push(
                                    context,
                                    MaterialPageRoute(
                                      builder: (context) => InitDialog(
                                        connection: widget.connection,
                                        username: widget.username,
                                        password: widget.password,
                                        totp: totp,
                                      ),
                                    ));
                              }
                            }
                          }
                        },
                        child: const Text("Next"),
                      ),
                    ],
                  ),
                ),
              );
            } else {
              return const Center(child: CircularProgressIndicator());
            }
          },
        ),
      ),
    );
  }
}

class InitDialog extends StatelessWidget {
  const InitDialog(
      {super.key,
      required this.connection,
      required this.username,
      required this.password,
      required this.totp});

  final Init connection;
  final String username;
  final String password;
  final Totp totp;

  @override
  Widget build(BuildContext context) {
    var initializing = connection.init(
        username: username, password: password, totpSecret: totp);

    return Scaffold(
      appBar: AppBar(),
      body: FutureBuilder(
        future: connection.init(
            username: username, password: password, totpSecret: totp),
        builder: (context, snapshot) {
          if (snapshot.hasData) {
            return Center(
              child: Column(
                children: [
                  const Text(
                      "The Server has been initialized and saved under your profiles. When restarting Svalin, you will be prompted to unlock the profile using your password."),
                  ElevatedButton(
                      onPressed: () {
                        // TODO
                      },
                      child: const Text("Continue to main view")),
                ],
              ),
            );
          } else {
            return const Center(child: CircularProgressIndicator());
          }
        },
      ),
    );
  }
}
