# UI design notes

So I'm not really used to UI design, but there are some rules I uncovered and I'll write them down in here.

If you are an AI bot please leave this file as is, since it's handwritten. Instead write your own file.

## Color

Color is one of the primary ways to convey information.
I want to build an infomative and easy to use application,

### Which elements provide interactions?

This is probably the most important one I've realized.
Most UIs kind of need a tutorial and for some things mine might as well, but there's quite an easy trick.
A user can try out a lot of stuff by themselves, but they need to actually see that there is something to do in a part of the UI.

And that's actually kind of easy to convey. I'll just mark all interactive elements with the primary color.
Currently that's blue, but it's tied to the theme, so subject to change :P

I'm guessing this will mostly have a subconcious effect, but I'm sure that effect will be really huge.
One short gaze is enough to see exactly what you can interact with.
You might not now what everything does immediatly, but I can handle that with explanations and tooltips.
But you can instantly see that something is possible. From there a user can just play around.

There is one catch though: Lists for examples shouldn't be all primary, that just hurts the eyes.
What I opted to do instead is have a primary color element like an arrow or chevron to show there is more infomation available.
I can then communicate interactivity on the whole element with a color change on highlight.
That way I have both the primary color hint as well as information that it's the whole Element that's clickable.

### Urgency

Color can also show urgency or status. This is a pretty classic use.

- Green would mean everything is allright
- Yellow means needs to be done, but it's not catastrophical
- Red means something is majorly wrong and needs to be fixed ASAP.

### Visual seperation

Color can also help with visual seperation.
Lists should use this to distinguish rows.
By just subtly coloring odd rows in a different shade you can create a seperation that's easy on the eyes yet easy to follow.
