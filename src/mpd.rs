use mpd_client::{
    commands::{responses as res, Command},
    filter::Filter,
    raw::RawCommand,
    tag::Tag,
};

pub(crate) struct Count {
    filter: Filter,
    group_by: Option<Tag>,
}

impl Count {
    pub(crate) fn new(filter: Filter) -> Self {
        Count {
            filter,
            group_by: None,
        }
    }

    pub(crate) fn group_by(mut self, group_by: Tag) -> Self {
        self.group_by = Some(group_by);
        self
    }
}

impl Command for Count {
    type Response = res::List;

    fn into_command(self) -> RawCommand {
        let mut command = RawCommand::new("count").argument(self.filter);

        if let Some(group_by) = self.group_by {
            command.add_argument("group").unwrap();
            command.add_argument(group_by).unwrap();
        }

        command
    }
}
