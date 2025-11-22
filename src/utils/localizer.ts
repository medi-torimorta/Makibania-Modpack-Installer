export type Language = "ja" | "en";

export interface Translation {
  alertOnLaunchModLoader: string;
  alertOnFailedAddProfile: string;
  alertOnFailedLaunchModLoader: string;
  appTitle: string;
  close: string;
  complete: string;
  install: string;
  installerTitle: string;
  languageOptionEn: string;
  languageOptionJa: string;
  languageSelectionLabel: string;
  occurredError: string;
  phaseAddProfile: string;
  phaseDownloadModLoader: string;
  phaseDownloadMods: string;
  phaseDownloadResources: string;
  phaseFinish: string;
  phaseLaunchModLoader: string;
  phaseStart: string;
  titleMessage: string;
}

export const translations: Record<Language, Translation> = {
  ja: {
    alertOnLaunchModLoader: "Modローダーが起動します。'クライアントをインストール/Install Client'にチェックが入っていることを確認の上、続行してください。",
    alertOnFailedAddProfile: "プロファイルの追加に失敗しました。Minecraftランチャーを起動し、手動で追加してください。",
    alertOnFailedLaunchModLoader: "Modローダーの起動に失敗しました。ダウンロードされたModローダーを手動で実行してください。",
    appTitle: "Makibania Modpack Installer",
    close: "閉じる",
    complete: "完了",
    install: "インストール",
    installerTitle: "インストーラー",
    languageOptionEn: "英語",
    languageOptionJa: "日本語",
    languageSelectionLabel: "表示言語",
    occurredError: "エラーが発生しました。\n詳細: ",
    phaseAddProfile: "プロファイルを追加中...",
    phaseDownloadModLoader: "Modローダーをダウンロード中...",
    phaseDownloadMods: "Modをダウンロード中...",
    phaseDownloadResources: "リソースをダウンロード中...",
    phaseFinish: "インストールが完了しました。",
    phaseLaunchModLoader: "Modローダーを起動中...",
    phaseStart: "インストールを開始しています...",
    titleMessage: "実行するモードを選択してください。",
  },
  en: {
    alertOnLaunchModLoader: "The mod loader will be launched. Please ensure that 'Install client' is checked, then click 'Next'.",
    alertOnFailedAddProfile: "Failed to add profile. Please launch the Minecraft launcher and add it manually.",
    alertOnFailedLaunchModLoader: "Failed to launch mod loader. Please run the downloaded mod loader manually.",
    appTitle: "Makibania Modpack Installer",
    close: "Close",
    complete: "Complete",
    install: "Install",
    installerTitle: "Installer",
    languageOptionEn: "English",
    languageOptionJa: "Japanese",
    languageSelectionLabel: "Display language",
    occurredError: "An error has occurred.\nDetails: ",
    phaseAddProfile: "Adding profile...",
    phaseDownloadModLoader: "Downloading mod loader...",
    phaseDownloadMods: "Downloading mods...",
    phaseDownloadResources: "Downloading resources...",
    phaseLaunchModLoader: "Launching mod loader...",
    phaseFinish: "Installation finished.",
    phaseStart: "Starting installation...",
    titleMessage: "Choose how you want to proceed.",
  },
};
