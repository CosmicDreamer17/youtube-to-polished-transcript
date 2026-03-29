# Transcript: Will AI replace programmers? | Lex Fridman Podcast

**Source:** https://youtu.be/odYNMCYRJS8
**Date transcribed:** 2026-03-29
**Speakers:** Lex Fridman, Nathan Lambert, Sebastian Raschka
**Duration:** 6 minutes

---

**Lex Fridman:** I think this has been happening over the last few weeks where people have gone from a month ago of oh yeah, agents are slop, which is a famous Carpathy quote, to the, what is a little bit of a meme of the industrialization of software when anyone can just create software at their fingerprints. I do think we are closer to that side of things and it takes direction and understanding how the systems work to extract that best from the language models. And I think it's hard to accept the gravity of how much is going to change with software development and how many more people can do things without ever looking at it.

**Nathan Lambert:** I think what's interesting is to think about whether these systems will be independent, completely independent in the sense that, well, I have no doubt that LLMs will at some point solve coding in a sense calculators solve calculating, right? So at some point humans developed a tool that, you never need a human to calculate that number. You just type it in and it's done. It's an algorithm, you can do it in that sense. And I think that's the same probably for coding. But the question is, so I think what will happen is, yeah, you will just say build that website, it will make a really good website and then you maybe refine it, but will it do things independently where, so will you be still having humans asking the AI to do something? will there be a person say build that website or will there be AI that just builds websites or something? Whatever.

**Sebastian Raschka:** I think using— talking about building websites is the—

**Lex Fridman:** too simple.

**Sebastian Raschka:** It's just there's the problem with websites and the problem with the web, HTML and all that stuff. It's very resilient to just slop. It will show you slop. It's good at showing us slop. I would rather think of safety-critical systems, asking AI to end-to-end generate something that manages logistics or manages cars, a fleet of cars, all that stuff. So end-to-end generates that for you.

**Lex Fridman:** I think a more intermediate example is take something Slack or Microsoft Word. I think if the organizations allow it, AI could very easily implement features end-to-end and do a fairly good job for things that you want to try. You want to add a new tab in Slack that you want to use, and I think AI will be able to do that pretty well.

**Sebastian Raschka:** Actually, that's a really great example. How far away are we from that?

**Lex Fridman:** This year.

**Sebastian Raschka:** See, I don't know. I don't know.

**Lex Fridman:** I guess I don't know how bad production code bases are, but I think that within on the order of low years, a lot of people are gonna be pushed to be more of a designer and product manager where you have multiple of these agents that can try things for you and they might take 1 to 2 days to implement a feature or attempt to fix a bug. And you have these dashboards, which I think Slack is actually a good dashboard where your agents will talk to you and you'll then give feedback, but things like I make a website, it's do you wanna make a logo that's passable? I think these cohesive design things and this style is gonna be very hard for models and deciding on what to add at the next time.

**Sebastian Raschka:** I just, okay, so I hang out with a lot of programmers and some of them are a little bit on the skeptical side in general. That's just vibe-wise, they're that. I just think there's a lot of complexity involved in adding features to complex systems. if you look at the browser Chrome, if I wanted to add a feature, if I wanted to have tabs as opposed to up top, I want 'em on the left side interface, right? I think we're not, this is not a next year thing.

**Lex Fridman:** One of the Claude releases this year, one of their tests was we give it a piece of software and leave Claude to run to recreate it entirely. And it could already almost rebuild scratch, Slack from scratch. Just given the parameters of the software and left in a sandbox environment.

**Sebastian Raschka:** So from the scratch part, I almost better.

**Lex Fridman:** So it might be that the smaller, newer companies are advantaged and they're we don't have to have the bloat and complexity and therefore this future exists.

**Nathan Lambert:** And I think this gets to the point that you mentioned that some people you talk to are skeptical. And I think that's not because the LLM can't do XYZ, it's because people don't want it to do it this way.

**Sebastian Raschka:** Some of that could be a skill issue on the human side. Unfortunately, we have to be honest with ourselves. And some of that could be an underspecification issue. So programming, you're just assuming this is in relationships and friendships, communication type of issue. You're assuming the LLM somehow is supposed to read your mind. I think this is where spec-driven design is really important. just using natural language, specify what you want.

**Lex Fridman:** I think that's if you talk to people at the labs, they use these in their training and production code. Claude code is built with Claude code, and they all use these things extensively. And Dario talks about how much of Claude's code own— it's these people are slightly ahead in terms of the capabilities they have, and they probably spend on inference— they could spend 10 to 100+X as much as we're spending. we're on a lowly $100 or $200 a month plan. they truly let it rip. And I think that with the pace of progress that we have, it seems a year ago we didn't have Claude code and we didn't really have reasoning models. And it's the difference between sitting here today and what we can do with these models. And it seems there's a lot of low-hanging fruit to improve them. The failure modes are pretty dumb. It's Claude, you tried to use this CLI command I don't have installed 14 times and then I sent you the command to run. It's that thing from a modeling perspective is pretty fixable. So I don't know.

**Sebastian Raschka:** I agree with you. I've been becoming more and more bullish in general. Speaking to what you're articulating, I think it is a human skill issue. So Anthropic is leading the way in, or other companies, in understanding how to best use the models for programming, therefore they're effectively using them. I think there's a lot of programmers on the outskirts that are they don't, I mean, there's not a really good guide on how to use them. People are trying to figure it out exactly.

**Lex Fridman:** It might be very expensive. it might be that the entry point for that is $2,000 a month, which is only tech companies and rich people, which is that could be it, but it might be worth it.

**Sebastian Raschka:** I mean, if the final result is a working software system, well, that'd be worth it. But by the way, it's funny how we converge from the discussion of timeline to AGI to something more pragmatic and useful.
