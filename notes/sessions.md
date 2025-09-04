# Sessions

While exploring MLS, I notized I can't have the same signer twice in a group.
Makes sense I guess, but that means I have to actually build the session system I've been thinking about but avoiding because I felt it's too complex.

## What is a session?

A session is just a short lived certificate that is signed directly by the user certificate and is specific to a device.
The login via PACE algorythim will stay, but what actually needs to be done for a login will change significantly.
This system heavily influences how the MLS group managment will work. I'll try to write my thoughts regarding this out here to sort them.

## Recap of Credentials and Users

An entity is one of the following:
- The Root (mostly a user too)
- A User
- A Server
- An Agent
- A session (this one is new)

Each entity is basically just a certificate with it's corresponding private key.
The SPKI-Hash (basically just a hash of the public key) is used as the identifier.
The reason is, that you could in theory fake a certificate for any Subject with another private key associated.
But you can't fake a certificate for the public key. The public and private keys are unique, so they're the perfect identifier.

So all I'm doing is handling standart certificates which basically just use their public key as subject.
Not much to it, so not much that should be able to break.

One additional rule is, that a certificate needs to be approved (basically just saved on) the server.
This is just a security measure so I always know which certificates "exist" in the way that if they're not recorded, they aren't valid.
I'd love to have some additional exploration of a system where the server can't hide a certificate here.
That's another topic that's probably pretty difficult though.

Back to sessions though:

The session in this matter is just a temporary certificate I'll use in place of the actual user certificate.
This is required for how I want to use MLS, but it also has an additional effect:

With those sessions I can see which devices are logged in and I can specifically log them our or log all devices out.
That won't even help against something like a keylogger, but it would probably help against stealing that data afterwards.
And it's just nice to see where you're logged in - I'd say that's a standart feature I'd expect to have in a "high security" setting.
Not that svalin will ever really compete in these environments. Convinience is more important in my eyes.

## What needs to happen on Login ?

### retrieve user certificate

Stage is in loggin in the the already known PACE algorithm to retrieve the certificate and private key.
Those are obviously encrypted and will be decrypted locally.

After this step I have the user certificate and it's private key - the users "credentials"

### Create a session and register it on the server

Next I'll have to create a session credential. The lifetime will be shorter - as of writing I'll try it with 30 days.
The session will be registered on the server, since it won't be accepted otherwise.
