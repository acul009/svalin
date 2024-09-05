import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:gui_flutter/src/app/main_menu.dart';
import 'package:gui_flutter/src/rust/api/client.dart';
import 'package:gui_flutter/src/rust/api/totp.dart';

class ServerDialog extends StatefulWidget {
  const ServerDialog({super.key});

  @override
  State<ServerDialog> createState() => _ServerDialogState();
}

class _ServerDialogState extends State<ServerDialog> {
  String _serverAddress = "";

  void _connect(BuildContext context) {
    Navigator.push(context, MaterialPageRoute(builder: (context) {
      return ConnectingDialog(address: _serverAddress);
    }));
  }

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
              onFieldSubmitted: (value) => _connect(context),
            ),
            const SizedBox(height: 20),
            ElevatedButton(
              style: ElevatedButton.styleFrom(
                minimumSize: const Size.fromHeight(50),
              ),
              child: const Text("Connect To Server"),
              onPressed: () => _connect(context),
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
          Navigator.pushAndRemoveUntil(
            context,
            MaterialPageRoute(
                builder: (context) => RegisterRootDialog(
                    connection: init, serverAddress: address)),
            (Route<dynamic> route) => false,
          );
        case FirstConnect_Login():
          showDialog(
            context: context,
            builder: (context) => const AlertDialog.adaptive(
              content: Text("Login is not ready yet!"),
            ),
          );
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
  const RegisterRootDialog(
      {super.key, required this.connection, required this.serverAddress});

  final Init connection;
  final String serverAddress;

  @override
  State<StatefulWidget> createState() {
    return _RegisterRootDialogState();
  }
}

class _RegisterRootDialogState extends State<RegisterRootDialog> {
  String _username = "";
  String _password = "";

  final _formKey = GlobalKey<FormState>();

  void _next(BuildContext context) {
    if (_formKey.currentState!.validate()) {
      Navigator.push(
          context,
          MaterialPageRoute(
            builder: (context) => CreateTotpDialog(
              connection: widget.connection,
              username: _username,
              password: _password,
              serverAddress: widget.serverAddress,
            ),
          ));
    }
  }

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
                onFieldSubmitted: (value) => _next(context),
              ),
              const SizedBox(height: 20),
              ElevatedButton(
                style: ElevatedButton.styleFrom(
                  minimumSize: const Size.fromHeight(50),
                ),
                onPressed: () {
                  _next(context);
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
      required this.password,
      required this.serverAddress});

  final Init connection;
  final String username;
  final String password;
  final String serverAddress;

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

  Future<void> _next(BuildContext context, Totp totp) async {
    if (_formKey.currentState!.validate()) {
      if (!await totp.checkCurrent(token: totpToken)) {
        if (context.mounted) {
          ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
            backgroundColor: Colors.red,
            content: Text("TOTP-token not valid.\nPlease try again!"),
          ));
        }
      } else {
        if (context.mounted) {
          Navigator.pushAndRemoveUntil(
            context,
            MaterialPageRoute(
              builder: (context) => InitDialog(
                connection: widget.connection,
                username: widget.username,
                password: widget.password,
                serverAddress: widget.serverAddress,
                totp: totp,
              ),
            ),
            (Route<dynamic> route) => false,
          );
        }
      }
    }
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
                          if (value == null || value.isEmpty) {
                            return "required";
                          } else if (!RegExp(r'^[0-9]{8}$').hasMatch(value)) {
                            return "not a TOTP token";
                          } else {
                            return null;
                          }
                        },
                        onFieldSubmitted: (value) async {
                          await _next(context, totp);
                        },
                      ),
                      const SizedBox(height: 20),
                      ElevatedButton(
                        style: ElevatedButton.styleFrom(
                          minimumSize: const Size.fromHeight(50),
                        ),
                        onPressed: () async {
                          await _next(context, totp);
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
      required this.totp,
      required this.serverAddress});

  final Init connection;
  final String username;
  final String password;
  final Totp totp;
  final String serverAddress;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(),
      body: FutureBuilder(
        future: connection.init(
            username: username, password: password, totpSecret: totp),
        builder: (context, snapshot) {
          if (snapshot.connectionState == ConnectionState.done) {
            if (snapshot.hasError) {
              return Center(child: ErrorWidget(snapshot.error!));
            } else {
              return Center(
                child: Column(
                  children: [
                    const Text(
                        "The Server has been initialized and saved under your profiles. When restarting Svalin, you will be prompted to unlock the profile using your password."),
                    ElevatedButton(
                        onPressed: () async {
                          var client = await Client.openProfileString(
                              profileKey: "$username@$serverAddress",
                              password: password);
                          Navigator.pushReplacement(context, MaterialPageRoute(
                            builder: (context) {
                              return MainMenu(client: client);
                            },
                          ));
                        },
                        child: const Text("Continue to main view")),
                  ],
                ),
              );
            }
          } else {
            return const Center(child: CircularProgressIndicator());
          }
        },
      ),
    );
  }
}
