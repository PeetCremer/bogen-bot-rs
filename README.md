# bogen-bot-rs 
A Discord bot written in Rust for "Burgen & Bullywugs", my own homebrewed tabletop role playing game (TTRPG).
The bot is designed to support online play within Discord together with your friends

## Key features
### Character Sheet Integration with 

To enable online play and rolling on character's abilities, the bot needs to get access to the character sheets that hold information about the character ability values. 

For that, the bot integrates with a [Google Spreadsheets](https://docs.google.com/spreadsheets/u/0/9), allowing you to manage and keep the character sheets for your group in there. Here is an ([example](https://docs.google.com/spreadsheets/d/1ZNN80gQ3PPUMSQY-YCeof9ZBAnPfvnpg784RoDLfdb0/edit?usp=sharing)), in german. 

For the bot to integrate with the spreadsheet, the spreadsheet should contain one table named after each character that holds that character's abilities. The names of the abilities should be in the `A` column, while the ability values should be in `G` column.

### Claiming characters on your sheet `!claim`
Once you have set up a character sheet with different tables for each character, players can claim a character using the `!claim <character name>` command. The character name must match the name of the table exactly, including case. 

After you have claimed a character, subsequent commands will assume that you are the player of that character and you will use the ability values from the corresponding table in the spreadsheet.

### Checking which character you have claimed `!my_character`
You can check which character you have claimed on the spreadsheet by using the `my_character` command.

### Rolling an ability for a claimed character `!check`
To roll an ability check for your claimed character, you can use the `!check` command followed by the name of an ability, or combination of abilities, you wish to roll. You don't have to type out the full name of the ability, just typing the first letters (including case) is enough, as long as a unique ability can be found that matches that.

To roll for `Charisma` for example, you can use `!check Chari`. This will roll two 4-sided dice (one positive and one negative) and adds the doubled value of your character's value in `Charisma` to the roll.

To roll for a combination of abilities, you can specify two abilities seperated by a ` + ` sign. Again, you don't need to write out the abilities and can abbreviate, as long as there is only one ability, respectively, that matches that. 
Example: `!check Chari + Strat` will roll two 4-sided dice (positive and negative) and will add to that your character's value in the `Charisma` and `Strategy` abilities.

### Rolling an ability for another character `!check_character`
Sometimes you have to roll an ability check for the character of another player that is absent or busy to keep. In this case, you can roll on that character's abilties without claiming it first, by using the `!check_character` command. The command works in the same way as the `!check` command, except that you need to specify the character name as the first argument. 

For example, to make an ability check for John's `Charisma`, you can use `!check_character John Charisma`

### Get help on commands using `!help` 
If you need help on any of the above command from within Discord, you can use the `!help` command

## Hosting the bot

### Prerequisites
To host the bot, you need to setup the following things
- A `DISCORD_BOT_TOKEN` that identifies the bot on the discord servers. [See this guide](https://discord.com/developers/docs/getting-started) for how to get started setting up a Discord Application and Bot
- `GOOGLE_APPLICATION_CREDENTIALS` (service account credentials) to interact with the Google spreadsheets API. [See here](https://developers.google.com/workspace/guides/create-credentials?hl=en) how to set those up
- A [Rust toolchain installation](https://rustup.rs/)
- The Google spreadsheet ID for the spreadsheet you have set up for your group
- An [SQLite installation](https://www.sqlite.org/download.html) to store the character sheet claims

### Setup
1. Clone the repo
2. Place a `.env` file with the following content in the repository root directory or store those values in your environment variables:
```
DISCORD_BOT_TOKEN='<YOUR DISCORD_BOT_TOKEN>'
CHARACTER_SPREADSHEET_ID='<SPREADSHEET ID FOR YOUR GROUP SPREADSHEET>'
GOOGLE_APPLICATION_CREDENTIALS='/home/myself/my_google_application_credentials.json'
``` 
3. Run `cargo build --release` to compile the bot
4. Run `cargo run --release` to run the bot


## Technical details
- This Bot uses the [serenity](https://docs.rs/serenity/latest/serenity/) library for Rust to talk asynchronously to the (https://support.discord.com/hc/en-us/articles/212889058-Discord-s-Official-API)[Discord API].
- Character sheet claims are stored in a simple [SQLite](https://www.sqlite.org/index.html) database
- Interaction with the Google Spreadsheets API uses the [google_sheets4][https://docs.rs/google-sheets4/latest/google_sheets4/9] Rust library. However, some hacks had to be added to enable use of the [gviz](https://developers.google.com/chart/interactive/docs/spreadsheets?hl=en) features for Google Spreadsheets, which make the lookup of ability values on the characters sheets much easier.
