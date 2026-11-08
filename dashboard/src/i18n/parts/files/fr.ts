import type { CatalogOf } from '../../types.ts';
import type en from './en.ts';

export default {
  // FileViewer: the page, its toolbar and dialogs
  'files.pageTitle': 'Explorateur de fichiers du serveur',
  'files.dropToUpload': 'Relâchez pour importer le fichier',
  'files.toolbar.addRemove': 'Ajouter/Retirer',
  'files.toolbar.selectAll': 'Tout sélectionner',
  'files.toolbar.deselectAll': 'Tout désélectionner',
  'files.toolbar.cutSelected': 'Couper la sélection',
  'files.toolbar.downloadSelected': 'Télécharger la sélection',
  'files.toolbar.unarchiveSelected': 'Extraire la sélection',
  'files.toolbar.deleteSelected': 'Supprimer la sélection',
  'files.toolbar.paste': {
    one: 'Coller {count} fichier',
    other: 'Coller {count} fichiers',
  },
  'files.toolbar.discardChanges': 'Annuler les modifications',
  'files.toolbar.upload': 'Importer',
  'files.cutToClipboard': {
    one: '{count} fichier coupé dans le presse-papiers',
    other: '{count} fichiers coupés dans le presse-papiers',
  },
  'files.zippingForDownload': 'Compression du dossier pour le téléchargement…',
  'files.downloadFailed': 'Échec du téléchargement de {name} : {error}',
  'files.onlyFilesUnzip': 'Seuls les fichiers peuvent être extraits',
  'files.onlyFilesUpload': 'Seuls des fichiers peuvent être importés',
  'files.delete.title': {
    one: 'Supprimer le fichier',
    other: 'Supprimer {count} fichiers',
  },
  'files.delete.message':
    'Voulez-vous vraiment supprimer les éléments suivants ?',
  'files.delete.directory': '(dossier)',
  'files.delete.unknownType': '(type inconnu)',
  'files.loadingFile': 'Chargement du fichier',
  'files.editor.title': 'Éditeur de fichiers',
  'files.editor.selectFile':
    'Sélectionnez un fichier pour afficher son contenu',
  'files.noReadPermission': 'Vous n’avez pas l’autorisation de lire ce dossier',
  'files.status.selected': {
    one: '{count} élément sélectionné',
    other: '{count} éléments sélectionnés',
  },
  'files.status.inClipboard': {
    one: '{count} élément dans le presse-papiers',
    other: '{count} éléments dans le presse-papiers',
  },

  // FileList: the list of files and folders
  'files.list.loading': 'Chargement des fichiers',
  'files.list.empty': 'Aucun fichier ici…',
  'files.list.unknownTime': 'Date de création inconnue',
  'files.list.folderIcon': 'icône de dossier',
  'files.list.fileIcon': 'icône de fichier',

  // FileContextMenu: the right-click menu of the file list
  'files.contextMenu.cut': 'Couper',
  'files.contextMenu.paste': 'Coller',
  'files.contextMenu.pasteInto': 'Coller dans {name}',
  'files.contextMenu.zip': 'Compresser',
  'files.contextMenu.zipTo': 'Compresser en {archive}',
  'files.contextMenu.zipFile': 'Compresser {name} en {archive}',
  'files.contextMenu.zipFiles': {
    one: 'Compresser {count} fichier en {archive}',
    other: 'Compresser {count} fichiers en {archive}',
  },
  'files.contextMenu.unzipHere': 'Extraire ici',
  'files.contextMenu.unzipHereSmart': 'Extraire ici (intelligent)',
  'files.contextMenu.unzipTo': 'Extraire dans {name}',
  'files.contextMenu.copiedToClipboard': {
    one: '{count} élément copié dans le presse-papiers',
    other: '{count} éléments copiés dans le presse-papiers',
  },
  'files.contextMenu.cutToClipboard': {
    one: '{count} élément coupé dans le presse-papiers',
    other: '{count} éléments coupés dans le presse-papiers',
  },
  'files.contextMenu.unsupportedZip': 'Type d’archive non pris en charge',
  'files.newFolder': 'Nouveau dossier',
  'files.newFile': 'Nouveau fichier',

  // CreateFileForm, CreateFolderForm, RenameFileForm: the dialogs
  'files.createFile.label': 'Nommez votre fichier',
  'files.createFile.placeholder': 'Sans titre',
  'files.createFile.submit': 'Créer le fichier',
  'files.createFolder.label': 'Nommez votre dossier',
  'files.createFolder.placeholder': 'Dossier sans titre',
  'files.createFolder.submit': 'Créer le dossier',
  'files.rename.label': 'Nommez votre fichier',
  'files.rename.submit': 'Renommer le fichier',

  // FileEditor
  'files.editor.loading': 'Chargement de l’éditeur',
} satisfies CatalogOf<typeof en>;
