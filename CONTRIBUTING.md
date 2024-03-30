# Contributing

Contributions to Flare are always welcome. There are many different ways you can contribute, this document will show you most of these ways and what you should keep in mind.

We are interested in getting to know our contributors and are often around to discuss the state and the future of Flare, be it something minor, or larger visions. Feel free to join our [Matrix channel](https://matrix.to/#/#flare-signal:matrix.org) and talk with us! This is also a good place to ask some questions or report smaller issues.

Note that the [GNOME Code of Conduct](https://wiki.gnome.org/Foundation/CodeOfConduct) applies to this project, therefore, be nice to each other.

## Translation

This is probably the easiest way to contribute to Flare. Just head over to [Weblate](https://hosted.weblate.org/projects/schmiddi-on-mobile/flare/) and start translating. If you are unfamiliar with Weblate, make sure to read their [documentation](https://docs.weblate.org/en/latest/user/translating.html) on how to translate a project.

## Open Issues

Issues are a good way to tell me problems you are having with the applications or things that you feel are missing or might be improved. There are a few things to keep in mind with issues.

- Use the correct template: This project defines a few different templates (e.g. "Feature Request", "Error") that you can use for different types of issues. Please only use the default template if you think no other template matches.
- Fill out all information in the template: For me to re-create your setup, it is important that you fill out all information that is asked for in the issue (e.g. your current version, or how you installed it).
- Fill out the logs if you think they are relevant: If the issue template asks for logs, and you think that might help, please consider adding them. But note that logs can contain sensitive information. Please look through the logs and censor things before pasting into the template (running the script [here](https://gitlab.com/schmiddi-on-mobile/flare/-/snippets/2538264) should censor your logs for you). If you don't feel comfortable censoring your own logs, you can also send me a Matrix direct message to `@schmiddi:matrix.org` if I ask for logs.
- Check for duplicated issues: Try to use the search-feature if you can find similar issues like you are having. If there is already such an issue, consider giving it a thumbs-up or commenting more details on that issue, but don't create a new issue.
- Know how to write a good issue: Read e.g. <https://wiredcraft.com/blog/how-we-write-our-github-issues/> (also applies pretty much got GitLab)

## Write Code

If you feel comfortable enough writing code, you can also submit your changes directly via a merge request.
You will need to know some Rust before contributing, and ideally also GTK.

If you want to implement any features Signal has, but Flare does not yet have, please check that presage supports that feature before working on it for Flare.
Even features that may sound simple, like read receipts, may not be feasible due to upstream not allowing for them.
When in doubt, please ask before getting started.

Here are a few pointers that might help write the code:

Review the [development-documentation](https://gitlab.com/schmiddi-on-mobile/flare/-/tree/master/doc?ref_type=heads) for information on how to compile the project, how the project is structured and other useful information.

